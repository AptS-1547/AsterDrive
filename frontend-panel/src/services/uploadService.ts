import { config } from "@/config/app";
import type { FileInfo } from "@/types/api";
import { api } from "./http";

export interface InitUploadResponse {
	mode: "direct" | "chunked" | "presigned";
	upload_id?: string;
	chunk_size?: number;
	total_chunks?: number;
	presigned_url?: string;
}

export interface ChunkUploadResponse {
	received_count: number;
	total_chunks: number;
}

export interface UploadProgressResponse {
	upload_id: string;
	status: string;
	received_count: number;
	chunks_on_disk: number[];
	total_chunks: number;
	filename: string;
}

export const uploadService = {
	initUpload: (data: {
		filename: string;
		total_size: number;
		folder_id?: number | null;
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

	completeUpload: (uploadId: string) =>
		api.post<FileInfo>(`/files/upload/${uploadId}/complete`),

	cancelUpload: (uploadId: string) =>
		api.delete<void>(`/files/upload/${uploadId}`),

	getProgress: (uploadId: string) =>
		api.get<UploadProgressResponse>(`/files/upload/${uploadId}`),

	/** PUT 直传 S3 presigned URL */
	presignedUpload: (
		presignedUrl: string,
		file: File,
		onProgress?: (loaded: number, total: number) => void,
	): Promise<void> => {
		return new Promise((resolve, reject) => {
			const xhr = new XMLHttpRequest();
			xhr.open("PUT", presignedUrl);
			xhr.setRequestHeader("Content-Type", "application/octet-stream");

			if (onProgress) {
				xhr.upload.onprogress = (e) => {
					if (e.lengthComputable) onProgress(e.loaded, e.total);
				};
			}

			xhr.onload = () => {
				if (xhr.status >= 200 && xhr.status < 300) {
					resolve();
				} else {
					reject(new Error(`S3 upload failed: ${xhr.status}`));
				}
			};
			xhr.onerror = () => reject(new Error("network error"));
			xhr.send(file);
		});
	},
};
