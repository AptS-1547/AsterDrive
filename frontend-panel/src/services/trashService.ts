import type { TrashContents } from "@/types/api";
import type { FolderListParams } from "./fileService";
import { api } from "./http";

export const trashService = {
	list: (params?: FolderListParams) =>
		api.get<TrashContents>("/trash", { params }),

	restoreFile: (id: number) => api.post<void>(`/trash/file/${id}/restore`),

	restoreFolder: (id: number) => api.post<void>(`/trash/folder/${id}/restore`),

	purgeFile: (id: number) => api.delete<void>(`/trash/file/${id}`),

	purgeFolder: (id: number) => api.delete<void>(`/trash/folder/${id}`),

	purgeAll: () => api.delete<void>("/trash"),
};
