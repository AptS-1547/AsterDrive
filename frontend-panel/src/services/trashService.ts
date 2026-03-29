import type { PurgedCountResponse, TrashContents } from "@/types/api";
import { api } from "./http";

export interface TrashListParams {
	folder_limit?: number;
	folder_offset?: number;
	file_limit?: number;
	file_after_deleted_at?: string;
	file_after_id?: number;
}

export const trashService = {
	list: (params?: TrashListParams) =>
		api.get<TrashContents>("/trash", { params }),

	restoreFile: (id: number) => api.post<void>(`/trash/file/${id}/restore`),

	restoreFolder: (id: number) => api.post<void>(`/trash/folder/${id}/restore`),

	purgeFile: (id: number) => api.delete<void>(`/trash/file/${id}`),

	purgeFolder: (id: number) => api.delete<void>(`/trash/folder/${id}`),

	purgeAll: () => api.delete<PurgedCountResponse>("/trash"),
};
