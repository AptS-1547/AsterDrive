import type { BatchResult } from "@/types/api";
import { api } from "@/services/http";

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
