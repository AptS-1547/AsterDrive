import { api } from "./http";
import type { FileInfo, FolderInfo, FolderContents } from "@/types/api";
import { config } from "@/config/app";

export const fileService = {
	listRoot: () => api.get<FolderContents>("/folders"),

	listFolder: (id: number) => api.get<FolderContents>(`/folders/${id}`),

	createFolder: (name: string, parentId?: number | null) =>
		api.post<FolderInfo>("/folders", { name, parent_id: parentId ?? null }),

	deleteFolder: (id: number) => api.delete<void>(`/folders/${id}`),

	renameFolder: (id: number, name: string) =>
		api.patch<FolderInfo>(`/folders/${id}`, { name }),

	getFile: (id: number) => api.get<FileInfo>(`/files/${id}`),

	deleteFile: (id: number) => api.delete<void>(`/files/${id}`),

	renameFile: (id: number, name: string) =>
		api.patch<FileInfo>(`/files/${id}`, { name }),

	downloadUrl: (id: number) => `${config.apiBaseUrl}/files/${id}/download`,

	thumbnailUrl: (id: number) => `${config.apiBaseUrl}/files/${id}/thumbnail`,

	setFileLock: (id: number, locked: boolean) =>
		api.post<FileInfo>(`/files/${id}/lock`, { locked }),

	setFolderLock: (id: number, locked: boolean) =>
		api.post<FolderInfo>(`/folders/${id}/lock`, { locked }),

	copyFile: (id: number, folderId?: number | null) =>
		api.post<FileInfo>(`/files/${id}/copy`, {
			folder_id: folderId ?? null,
		}),

	copyFolder: (id: number, parentId?: number | null) =>
		api.post<FolderInfo>(`/folders/${id}/copy`, {
			parent_id: parentId ?? null,
		}),

	listVersions: (id: number) => api.get<FileVersion[]>(`/files/${id}/versions`),

	restoreVersion: (fileId: number, versionId: number) =>
		api.post<FileInfo>(`/files/${fileId}/versions/${versionId}/restore`),

	deleteVersion: (fileId: number, versionId: number) =>
		api.delete<void>(`/files/${fileId}/versions/${versionId}`),
};

export interface FileVersion {
	id: number;
	file_id: number;
	blob_id: number;
	version: number;
	size: number;
	created_at: string;
}
