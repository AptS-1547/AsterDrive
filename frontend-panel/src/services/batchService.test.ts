import { beforeEach, describe, expect, it, vi } from "vitest";
import { batchService, createBatchService } from "@/services/batchService";

const apiPost = vi.hoisted(() => vi.fn());

vi.mock("@/services/http", () => ({
	api: {
		post: apiPost,
	},
}));

describe("batchService", () => {
	beforeEach(() => {
		apiPost.mockReset();
	});

	it("posts delete, move, and copy batch payloads", () => {
		batchService.batchDelete([1, 2], [3]);
		batchService.batchMove([1], [2, 3], 9);
		batchService.batchCopy([], [4], null);

		expect(apiPost).toHaveBeenNthCalledWith(1, "/batch/delete", {
			file_ids: [1, 2],
			folder_ids: [3],
		});
		expect(apiPost).toHaveBeenNthCalledWith(2, "/batch/move", {
			file_ids: [1],
			folder_ids: [2, 3],
			target_folder_id: 9,
		});
		expect(apiPost).toHaveBeenNthCalledWith(3, "/batch/copy", {
			file_ids: [],
			folder_ids: [4],
			target_folder_id: null,
		});

		const teamBatchService = createBatchService({ kind: "team", teamId: 4 });
		teamBatchService.batchDelete([1], []);
		teamBatchService.batchMove([], [2], 8);
		teamBatchService.batchCopy([3], [], null);

		expect(apiPost).toHaveBeenNthCalledWith(4, "/teams/4/batch/delete", {
			file_ids: [1],
			folder_ids: [],
		});
		expect(apiPost).toHaveBeenNthCalledWith(5, "/teams/4/batch/move", {
			file_ids: [],
			folder_ids: [2],
			target_folder_id: 8,
		});
		expect(apiPost).toHaveBeenNthCalledWith(6, "/teams/4/batch/copy", {
			file_ids: [3],
			folder_ids: [],
			target_folder_id: null,
		});
	});
});
