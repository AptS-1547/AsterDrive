import {
	PERSONAL_WORKSPACE,
	type Workspace,
	workspaceApiPrefix,
} from "@/lib/workspace";
import { api } from "@/services/http";
import { bindWorkspaceService } from "@/stores/workspaceStore";
import type { BatchResult } from "@/types/api";

function buildPath(workspace: Workspace, path: string) {
	return `${workspaceApiPrefix(workspace)}${path}`;
}

export function createBatchService(workspace: Workspace = PERSONAL_WORKSPACE) {
	return {
		batchDelete: (fileIds: number[], folderIds: number[]) =>
			api.post<BatchResult>(buildPath(workspace, "/batch/delete"), {
				file_ids: fileIds,
				folder_ids: folderIds,
			}),

		batchMove: (
			fileIds: number[],
			folderIds: number[],
			targetFolderId: number | null,
		) =>
			api.post<BatchResult>(buildPath(workspace, "/batch/move"), {
				file_ids: fileIds,
				folder_ids: folderIds,
				target_folder_id: targetFolderId,
			}),

		batchCopy: (
			fileIds: number[],
			folderIds: number[],
			targetFolderId: number | null,
		) =>
			api.post<BatchResult>(buildPath(workspace, "/batch/copy"), {
				file_ids: fileIds,
				folder_ids: folderIds,
				target_folder_id: targetFolderId,
			}),
	};
}

export const batchService = bindWorkspaceService(createBatchService);
