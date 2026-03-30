import { config } from "@/config/app";
import type {
	BatchResult,
	FolderContents,
	ShareInfo,
	SharePage,
	SharePublicInfo,
} from "@/types/api";
import type { FolderListParams } from "./fileService";
import { api } from "./http";

export const shareService = {
	create: (data: {
		file_id?: number;
		folder_id?: number;
		password?: string;
		expires_at?: string;
		max_downloads?: number;
	}) => api.post<ShareInfo>("/shares", data),

	listMine: (params?: { limit?: number; offset?: number }) =>
		api.get<SharePage>("/shares", { params }),

	update: (
		id: number,
		data: {
			password?: string;
			expires_at: string | null;
			max_downloads: number;
		},
	) => api.patch<ShareInfo>(`/shares/${id}`, data),

	delete: (id: number) => api.delete<void>(`/shares/${id}`),

	batchDelete: (shareIds: number[]) =>
		api.post<BatchResult>("/shares/batch-delete", {
			share_ids: shareIds,
		}),

	getInfo: (token: string) => api.get<SharePublicInfo>(`/s/${token}`),

	verifyPassword: (token: string, password: string) =>
		api.post<void>(`/s/${token}/verify`, { password }),

	downloadPath: (token: string) => `/s/${token}/download`,

	thumbnailPath: (token: string) => `/s/${token}/thumbnail`,

	downloadFolderPath: (token: string, fileId: number) =>
		`/s/${token}/files/${fileId}/download`,

	downloadUrl: (token: string) => `${config.apiBaseUrl}/s/${token}/download`,

	downloadFolderFileUrl: (token: string, fileId: number) =>
		`${config.apiBaseUrl}/s/${token}/files/${fileId}/download`,

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
