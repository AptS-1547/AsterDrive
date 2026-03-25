import type { FolderContents, ShareInfo, SharePublicInfo } from "@/types/api";
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

	listMine: () => api.get<ShareInfo[]>("/shares"),

	delete: (id: number) => api.delete<void>(`/shares/${id}`),

	getInfo: (token: string) => api.get<SharePublicInfo>(`/s/${token}`),

	verifyPassword: (token: string, password: string) =>
		api.post<null>(`/s/${token}/verify`, { password }),

	downloadUrl: (token: string) => `/s/${token}/download`,

	downloadFolderFileUrl: (token: string, fileId: number) =>
		`/s/${token}/files/${fileId}/download`,

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
