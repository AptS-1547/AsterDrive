import { config } from "@/config/app";
import type {
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

	delete: (id: number) => api.delete<void>(`/shares/${id}`),

	getInfo: (token: string) => api.get<SharePublicInfo>(`/s/${token}`),

	verifyPassword: (token: string, password: string) =>
		api.post<null>(`/s/${token}/verify`, { password }),

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
