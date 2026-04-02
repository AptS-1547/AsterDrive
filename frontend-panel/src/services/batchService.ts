import {
	buildWorkspacePath,
	PERSONAL_WORKSPACE,
	type Workspace,
} from "@/lib/workspace";
import { api } from "@/services/http";
import { bindWorkspaceService } from "@/stores/workspaceStore";
import type { BatchResult } from "@/types/api";

export function createBatchService(workspace: Workspace = PERSONAL_WORKSPACE) {
	return {
		batchDelete: (fileIds: number[], folderIds: number[]) =>
			api.post<BatchResult>(buildWorkspacePath(workspace, "/batch/delete"), {
				file_ids: fileIds,
				folder_ids: folderIds,
			}),

		batchMove: (
			fileIds: number[],
			folderIds: number[],
			targetFolderId: number | null,
		) =>
			api.post<BatchResult>(buildWorkspacePath(workspace, "/batch/move"), {
				file_ids: fileIds,
				folder_ids: folderIds,
				target_folder_id: targetFolderId,
			}),

		batchCopy: (
			fileIds: number[],
			folderIds: number[],
			targetFolderId: number | null,
		) =>
			api.post<BatchResult>(buildWorkspacePath(workspace, "/batch/copy"), {
				file_ids: fileIds,
				folder_ids: folderIds,
				target_folder_id: targetFolderId,
			}),
	};
}

export const batchService = bindWorkspaceService(createBatchService);
