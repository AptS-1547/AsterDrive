import { api } from "./http";
import type { TrashContents } from "@/types/api";

export const trashService = {
	list: () => api.get<TrashContents>("/trash"),

	restoreFile: (id: number) => api.post<void>(`/trash/file/${id}/restore`),

	restoreFolder: (id: number) => api.post<void>(`/trash/folder/${id}/restore`),

	purgeFile: (id: number) => api.delete<void>(`/trash/file/${id}`),

	purgeFolder: (id: number) => api.delete<void>(`/trash/folder/${id}`),

	purgeAll: () => api.delete<void>("/trash"),
};
