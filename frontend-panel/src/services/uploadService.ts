import { config } from "@/config/app";
import type {
	ChunkUploadResponse,
	CompletedPart,
	FileInfo,
	InitUploadResponse,
	UploadProgressResponse,
} from "@/types/api";
import { api } from "./http";

export type {
	ChunkUploadResponse,
	CompletedPart,
	InitUploadResponse,
	UploadProgressResponse,
};

export const uploadService = {
	initUpload: (data: {
		filename: string;
		total_size: number;
		folder_id?: number | null;
		relative_path?: string;
	}) => api.post<InitUploadResponse>("/files/upload/init", data),

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
				`${config.apiBaseUrl}/files/upload/${uploadId}/${chunkNumber}`,
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
						reject(new Error(resp.msg));
					}
				} else {
					reject(new Error(`chunk upload failed: ${xhr.status}`));
				}
			};
			xhr.onerror = () => reject(new Error("network error"));
			xhr.send(data);
		});
	},

	completeUpload: (uploadId: string, parts?: CompletedPart[]) =>
		api.post<FileInfo>(
			`/files/upload/${uploadId}/complete`,
			parts ? { parts } : undefined,
		),

	cancelUpload: (uploadId: string) =>
		api.delete<void>(`/files/upload/${uploadId}`),

	getProgress: (uploadId: string) =>
		api.get<UploadProgressResponse>(`/files/upload/${uploadId}`),

	/** 批量获取 S3 multipart part presigned URLs */
	presignParts: (uploadId: string, partNumbers: number[]) =>
		api.post<Record<number, string>>(
			`/files/upload/${uploadId}/presign-parts`,
			{
				part_numbers: partNumbers,
			},
		),

	/** PUT 直传 S3 presigned URL（单次 PUT，用于小文件或单个 part） */
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
					// S3 returns ETag in response header
					// CORS must expose ETag: bucket config needs ExposeHeaders: ["ETag"]
					const etag = xhr.getResponseHeader("ETag") ?? "";
					if (!etag) {
						reject(
							new Error(
								"S3 did not return ETag header. Check bucket CORS ExposeHeaders configuration.",
							),
						);
						return;
					}
					resolve(etag);
				} else {
					reject(new Error(`S3 upload failed: ${xhr.status}`));
				}
			};
			xhr.onerror = () => reject(new Error("network error"));
			xhr.send(file);
		});
	},
};
