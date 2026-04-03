import { config } from "@/config/app";
import { joinApiUrl } from "@/lib/apiUrl";
import { buildWorkspacePath, type Workspace } from "@/lib/workspace";
import { bindWorkspaceService } from "@/stores/workspaceStore";
import type {
	ErrorCode,
	FileInfo,
	FileVersion,
	FolderAncestorItem,
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

export function createFileService(workspace: Workspace) {
	return {
		listRoot: (params?: FolderListParams) =>
			api.get<FolderContents>(buildWorkspacePath(workspace, "/folders"), {
				params,
			}),

		listFolder: (id: number, params?: FolderListParams) =>
			api.get<FolderContents>(buildWorkspacePath(workspace, `/folders/${id}`), {
				params,
			}),

		getFolderAncestors: (id: number) =>
			api.get<FolderAncestorItem[]>(
				buildWorkspacePath(workspace, `/folders/${id}/ancestors`),
			),

		createFolder: (name: string, parentId?: number | null) =>
			api.post<FolderInfo>(buildWorkspacePath(workspace, "/folders"), {
				name,
				parent_id: parentId ?? null,
			}),

		deleteFolder: (id: number) =>
			api.delete<void>(buildWorkspacePath(workspace, `/folders/${id}`)),

		renameFolder: (id: number, name: string) =>
			api.patch<FolderInfo>(buildWorkspacePath(workspace, `/folders/${id}`), {
				name,
			}),

		getFile: (id: number) =>
			api.get<FileInfo>(buildWorkspacePath(workspace, `/files/${id}`)),

		deleteFile: (id: number) =>
			api.delete<void>(buildWorkspacePath(workspace, `/files/${id}`)),

		renameFile: (id: number, name: string) =>
			api.patch<FileInfo>(buildWorkspacePath(workspace, `/files/${id}`), {
				name,
			}),

		downloadPath: (id: number) =>
			buildWorkspacePath(workspace, `/files/${id}/download`),

		downloadUrl: (id: number) =>
			joinApiUrl(
				config.apiBaseUrl,
				buildWorkspacePath(workspace, `/files/${id}/download`),
			),

		thumbnailPath: (id: number) =>
			buildWorkspacePath(workspace, `/files/${id}/thumbnail`),

		setFileLock: (id: number, locked: boolean) =>
			api.post<FileInfo>(buildWorkspacePath(workspace, `/files/${id}/lock`), {
				locked,
			}),

		setFolderLock: (id: number, locked: boolean) =>
			api.post<FolderInfo>(
				buildWorkspacePath(workspace, `/folders/${id}/lock`),
				{
					locked,
				},
			),

		createEmptyFile: (name: string, folderId?: number | null) =>
			api.post<FileInfo>(buildWorkspacePath(workspace, "/files/new"), {
				name,
				folder_id: folderId ?? null,
			}),

		copyFile: (id: number, folderId?: number | null) =>
			api.post<FileInfo>(buildWorkspacePath(workspace, `/files/${id}/copy`), {
				folder_id: folderId ?? null,
			}),

		copyFolder: (id: number, parentId?: number | null) =>
			api.post<FolderInfo>(
				buildWorkspacePath(workspace, `/folders/${id}/copy`),
				{
					parent_id: parentId ?? null,
				},
			),

		updateContent: async (id: number, content: string, etag?: string) => {
			const headers: Record<string, string> = {
				"Content-Type": "application/octet-stream",
			};
			if (etag) headers["If-Match"] = etag;
			try {
				const resp = await api.client.put(
					buildWorkspacePath(workspace, `/files/${id}/content`),
					content,
					{
						headers,
					},
				);
				return resp.data.data as FileInfo;
			} catch (err: unknown) {
				if (err && typeof err === "object") {
					const response = (
						err as {
							response?: {
								status: number;
								data?: { code?: number; msg?: string };
							} | null;
						}
					).response;
					if (response != null) {
						const status = response.status;
						const body = response.data;
						const apiErr = new ApiError(
							(body?.code ?? status) as ErrorCode,
							body?.msg ?? `HTTP ${status}`,
						);
						(apiErr as ApiError & { status: number }).status = status;
						throw apiErr;
					}
				}
				throw err;
			}
		},

		listVersions: (id: number) =>
			api.get<FileVersion[]>(
				buildWorkspacePath(workspace, `/files/${id}/versions`),
			),

		restoreVersion: (fileId: number, versionId: number) =>
			api.post<FileInfo>(
				buildWorkspacePath(
					workspace,
					`/files/${fileId}/versions/${versionId}/restore`,
				),
			),

		deleteVersion: (fileId: number, versionId: number) =>
			api.delete<void>(
				buildWorkspacePath(workspace, `/files/${fileId}/versions/${versionId}`),
			),
	};
}

// `fileService` methods resolve the current workspace when invoked, so cached
// or destructured method references still follow workspace changes. Use
// `createFileService(workspace)` for an explicit stable workspace instance.
export const fileService = bindWorkspaceService(createFileService);
