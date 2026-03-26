import type {
	ErrorCode,
	FileInfo,
	FileVersion,
	FolderContents,
	FolderInfo,
} from "@/types/api";
import { ApiError, api } from "./http";

export interface FolderListParams {
	folder_limit?: number;
	folder_offset?: number;
	file_limit?: number;
	file_after_value?: string;
	file_after_id?: number;
	sort_by?: "name" | "size" | "created_at" | "updated_at" | "type";
	sort_order?: "asc" | "desc";
}

export const fileService = {
	listRoot: (params?: FolderListParams) =>
		api.get<FolderContents>("/folders", { params }),

	listFolder: (id: number, params?: FolderListParams) =>
		api.get<FolderContents>(`/folders/${id}`, { params }),

	createFolder: (name: string, parentId?: number | null) =>
		api.post<FolderInfo>("/folders", { name, parent_id: parentId ?? null }),

	deleteFolder: (id: number) => api.delete<void>(`/folders/${id}`),

	renameFolder: (id: number, name: string) =>
		api.patch<FolderInfo>(`/folders/${id}`, { name }),

	getFile: (id: number) => api.get<FileInfo>(`/files/${id}`),

	deleteFile: (id: number) => api.delete<void>(`/files/${id}`),

	renameFile: (id: number, name: string) =>
		api.patch<FileInfo>(`/files/${id}`, { name }),

	downloadPath: (id: number) => `/files/${id}/download`,

	thumbnailPath: (id: number) => `/files/${id}/thumbnail`,

	setFileLock: (id: number, locked: boolean) =>
		api.post<FileInfo>(`/files/${id}/lock`, { locked }),

	setFolderLock: (id: number, locked: boolean) =>
		api.post<FolderInfo>(`/folders/${id}/lock`, { locked }),

	createEmptyFile: (name: string, folderId?: number | null) =>
		api.post<FileInfo>("/files/new", { name, folder_id: folderId ?? null }),

	copyFile: (id: number, folderId?: number | null) =>
		api.post<FileInfo>(`/files/${id}/copy`, {
			folder_id: folderId ?? null,
		}),

	copyFolder: (id: number, parentId?: number | null) =>
		api.post<FolderInfo>(`/folders/${id}/copy`, {
			parent_id: parentId ?? null,
		}),

	updateContent: async (id: number, content: string, etag?: string) => {
		const headers: Record<string, string> = {
			"Content-Type": "application/octet-stream",
		};
		if (etag) headers["If-Match"] = etag;
		try {
			const resp = await api.client.put(`/files/${id}/content`, content, {
				headers,
			});
			return resp.data.data as FileInfo;
		} catch (err: unknown) {
			if (err && typeof err === "object" && "response" in err) {
				const axiosErr = err as {
					response: { status: number; data?: { code?: number; msg?: string } };
				};
				const status = axiosErr.response.status;
				const body = axiosErr.response.data;
				const apiErr = new ApiError(
					(body?.code ?? status) as ErrorCode,
					body?.msg ?? `HTTP ${status}`,
				);
				(apiErr as ApiError & { status: number }).status = status;
				throw apiErr;
			}
			throw err;
		}
	},

	listVersions: (id: number) => api.get<FileVersion[]>(`/files/${id}/versions`),

	restoreVersion: (fileId: number, versionId: number) =>
		api.post<FileInfo>(`/files/${fileId}/versions/${versionId}/restore`),

	deleteVersion: (fileId: number, versionId: number) =>
		api.delete<void>(`/files/${fileId}/versions/${versionId}`),
};
