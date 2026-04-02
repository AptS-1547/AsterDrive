import { config } from "@/config/app";
import {
	PERSONAL_WORKSPACE,
	type Workspace,
	workspaceApiPrefix,
} from "@/lib/workspace";
import { bindWorkspaceService } from "@/stores/workspaceStore";
import type {
	ChunkUploadResponse,
	CompletedPart,
	FileInfo,
	InitUploadResponse,
	UploadProgressResponse,
} from "@/types/api";
import { type ApiResponse, ErrorCode } from "@/types/api-helpers";
import { ApiError, api } from "./http";

export type {
	ChunkUploadResponse,
	CompletedPart,
	InitUploadResponse,
	UploadProgressResponse,
};

export class UploadRequestError extends Error {
	retryable: boolean;
	status?: number;

	constructor(
		message: string,
		options?: {
			retryable?: boolean;
			status?: number;
		},
	) {
		super(message);
		this.name = "UploadRequestError";
		this.retryable = options?.retryable ?? false;
		this.status = options?.status;
	}
}

function isRetryableHttpStatus(status: number): boolean {
	return status === 408 || status === 429 || status >= 500;
}

function parseApiMessage(responseText: string): string | null {
	if (!responseText) return null;
	try {
		const parsed = JSON.parse(responseText) as { msg?: string };
		return typeof parsed.msg === "string" ? parsed.msg : null;
	} catch {
		return null;
	}
}

export function isRetryableUploadError(error: unknown): boolean {
	return (
		typeof error === "object" &&
		error !== null &&
		"retryable" in error &&
		(error as { retryable?: boolean }).retryable === true
	);
}

function uploadPath(workspace: Workspace, path: string) {
	return `${workspaceApiPrefix(workspace)}${path}`;
}

export function createUploadService(workspace: Workspace = PERSONAL_WORKSPACE) {
	return {
		initUpload: (data: {
			filename: string;
			total_size: number;
			folder_id?: number | null;
			relative_path?: string;
		}) =>
			api.post<InitUploadResponse>(
				uploadPath(workspace, "/files/upload/init"),
				data,
			),

		uploadChunk: (
			uploadId: string,
			chunkNumber: number,
			data: Blob,
			onProgress?: (loaded: number, total: number) => void,
		): Promise<ChunkUploadResponse> => {
			return new Promise((resolve, reject) => {
				const xhr = new XMLHttpRequest();
				xhr.open(
					"PUT",
					`${config.apiBaseUrl}${uploadPath(
						workspace,
						`/files/upload/${uploadId}/${chunkNumber}`,
					)}`,
				);
				xhr.withCredentials = true;
				xhr.setRequestHeader("Content-Type", "application/octet-stream");

				if (onProgress) {
					xhr.upload.onprogress = (e) => {
						if (e.lengthComputable) onProgress(e.loaded, e.total);
					};
				}

				xhr.onload = () => {
					if (xhr.status >= 200 && xhr.status < 300) {
						const resp = JSON.parse(xhr.responseText);
						if (resp.code === 0) {
							resolve(resp.data);
						} else {
							reject(
								new UploadRequestError(resp.msg, {
									status: xhr.status,
									retryable: false,
								}),
							);
						}
					} else {
						reject(
							new UploadRequestError(
								parseApiMessage(xhr.responseText) ??
									`chunk upload failed: ${xhr.status}`,
								{
									status: xhr.status,
									retryable: isRetryableHttpStatus(xhr.status),
								},
							),
						);
					}
				};
				xhr.onerror = () =>
					reject(
						new UploadRequestError("network error", {
							retryable: true,
						}),
					);
				xhr.send(data);
			});
		},

		completeUpload: async (
			uploadId: string,
			parts?: CompletedPart[],
		): Promise<FileInfo> => {
			const resp = await api.client.post<ApiResponse<FileInfo>>(
				uploadPath(workspace, `/files/upload/${uploadId}/complete`),
				parts ? { parts } : undefined,
				{ timeout: 0 },
			);
			if (resp.data.code !== ErrorCode.Success) {
				throw new ApiError(resp.data.code, resp.data.msg);
			}
			return resp.data.data as FileInfo;
		},

		cancelUpload: (uploadId: string) =>
			api.delete<void>(uploadPath(workspace, `/files/upload/${uploadId}`)),

		getProgress: (uploadId: string) =>
			api.get<UploadProgressResponse>(
				uploadPath(workspace, `/files/upload/${uploadId}`),
			),

		presignParts: (uploadId: string, partNumbers: number[]) =>
			api.post<Record<number, string>>(
				uploadPath(workspace, `/files/upload/${uploadId}/presign-parts`),
				{
					part_numbers: partNumbers,
				},
			),

		presignedUpload: (
			presignedUrl: string,
			file: File | Blob,
			onProgress?: (loaded: number, total: number) => void,
			onCreateXhr?: (xhr: XMLHttpRequest) => void,
		): Promise<string> => {
			return new Promise((resolve, reject) => {
				const xhr = new XMLHttpRequest();
				onCreateXhr?.(xhr);
				xhr.open("PUT", presignedUrl);
				xhr.setRequestHeader("Content-Type", "application/octet-stream");

				if (onProgress) {
					xhr.upload.onprogress = (e) => {
						if (e.lengthComputable) onProgress(e.loaded, e.total);
					};
				}

				xhr.onload = () => {
					if (xhr.status >= 200 && xhr.status < 300) {
						const etag = xhr.getResponseHeader("ETag") ?? "";
						if (!etag) {
							reject(
								new UploadRequestError(
									"S3 did not return ETag header. Check bucket CORS ExposeHeaders configuration.",
									{ status: xhr.status, retryable: false },
								),
							);
							return;
						}
						resolve(etag);
					} else {
						reject(
							new UploadRequestError(
								parseApiMessage(xhr.responseText) ??
									`S3 upload failed: ${xhr.status}`,
								{
									status: xhr.status,
									retryable: isRetryableHttpStatus(xhr.status),
								},
							),
						);
					}
				};
				xhr.onerror = () =>
					reject(
						new UploadRequestError("network error", {
							retryable: true,
						}),
					);
				xhr.send(file);
			});
		},
	};
}

export const uploadService = bindWorkspaceService(createUploadService);
