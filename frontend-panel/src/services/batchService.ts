import { api } from "@/services/http";

interface BatchResult {
	succeeded: number;
	failed: number;
	errors: Array<{ entity_type: string; entity_id: number; error: string }>;
}

export const batchService = {
	batchDelete: (fileIds: number[], folderIds: number[]) =>
		api.post<BatchResult>("/batch/delete", {
			file_ids: fileIds,
			folder_ids: folderIds,
		}),

	batchMove: (
		fileIds: number[],
		folderIds: number[],
		targetFolderId: number | null,
	) =>
		api.post<BatchResult>("/batch/move", {
			file_ids: fileIds,
			folder_ids: folderIds,
			target_folder_id: targetFolderId,
		}),

	batchCopy: (
		fileIds: number[],
		folderIds: number[],
		targetFolderId: number | null,
	) =>
		api.post<BatchResult>("/batch/copy", {
			file_ids: fileIds,
			folder_ids: folderIds,
			target_folder_id: targetFolderId,
		}),
};
