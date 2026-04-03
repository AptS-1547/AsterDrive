import { config } from "@/config/app";
import { joinApiUrl } from "@/lib/apiUrl";
import {
	buildWorkspacePath,
	PERSONAL_WORKSPACE,
	type Workspace,
} from "@/lib/workspace";
import { bindWorkspaceService } from "@/stores/workspaceStore";
import type {
	BatchResult,
	FolderContents,
	ShareInfo,
	SharePage,
	SharePublicInfo,
} from "@/types/api";
import type { FolderListParams } from "./fileService";
import { api } from "./http";

function workspaceSharesPrefix(workspace: Workspace) {
	return buildWorkspacePath(workspace, "/shares");
}

export function createShareService(workspace: Workspace = PERSONAL_WORKSPACE) {
	return {
		create: (data: {
			file_id?: number;
			folder_id?: number;
			password?: string;
			expires_at?: string;
			max_downloads?: number;
		}) => api.post<ShareInfo>(workspaceSharesPrefix(workspace), data),

		listMine: (params?: { limit?: number; offset?: number }) =>
			api.get<SharePage>(workspaceSharesPrefix(workspace), { params }),

		update: (
			id: number,
			data: {
				password?: string;
				expires_at: string | null;
				max_downloads: number;
			},
		) =>
			api.patch<ShareInfo>(`${workspaceSharesPrefix(workspace)}/${id}`, data),

		delete: (id: number) =>
			api.delete<void>(`${workspaceSharesPrefix(workspace)}/${id}`),

		batchDelete: (shareIds: number[]) =>
			api.post<BatchResult>(
				`${workspaceSharesPrefix(workspace)}/batch-delete`,
				{
					share_ids: shareIds,
				},
			),

		getInfo: (token: string) => api.get<SharePublicInfo>(`/s/${token}`),

		verifyPassword: (token: string, password: string) =>
			api.post<void>(`/s/${token}/verify`, { password }),

		downloadPath: (token: string) => `/s/${token}/download`,

		thumbnailPath: (token: string) => `/s/${token}/thumbnail`,

		downloadFolderPath: (token: string, fileId: number) =>
			`/s/${token}/files/${fileId}/download`,

		downloadUrl: (token: string) =>
			joinApiUrl(config.apiBaseUrl, `/s/${token}/download`),

		downloadFolderFileUrl: (token: string, fileId: number) =>
			joinApiUrl(config.apiBaseUrl, `/s/${token}/files/${fileId}/download`),

		listContent: (token: string, params?: FolderListParams) =>
			api.get<FolderContents>(`/s/${token}/content`, { params }),

		listSubfolderContent: (
			token: string,
			folderId: number,
			params?: FolderListParams,
		) =>
			api.get<FolderContents>(`/s/${token}/folders/${folderId}/content`, {
				params,
			}),
	};
}

export const shareService = bindWorkspaceService(createShareService);
