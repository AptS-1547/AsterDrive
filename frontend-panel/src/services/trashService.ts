import {
	buildWorkspacePath,
	PERSONAL_WORKSPACE,
	type Workspace,
} from "@/lib/workspace";
import { bindWorkspaceService } from "@/stores/workspaceStore";
import type { PurgedCountResponse, TrashContents } from "@/types/api";
import { api } from "./http";

export interface TrashListParams {
	folder_limit?: number;
	folder_offset?: number;
	file_limit?: number;
	file_after_deleted_at?: string;
	file_after_id?: number;
}

function trashPath(workspace: Workspace) {
	return buildWorkspacePath(workspace, "/trash");
}

export function createTrashService(workspace: Workspace = PERSONAL_WORKSPACE) {
	const basePath = trashPath(workspace);
	return {
		list: (params?: TrashListParams) =>
			api.get<TrashContents>(basePath, { params }),

		restoreFile: (id: number) =>
			api.post<void>(`${basePath}/file/${id}/restore`),

		restoreFolder: (id: number) =>
			api.post<void>(`${basePath}/folder/${id}/restore`),

		purgeFile: (id: number) => api.delete<void>(`${basePath}/file/${id}`),

		purgeFolder: (id: number) => api.delete<void>(`${basePath}/folder/${id}`),

		purgeAll: () => api.delete<PurgedCountResponse>(basePath),
	};
}

export const trashService = bindWorkspaceService(createTrashService);
