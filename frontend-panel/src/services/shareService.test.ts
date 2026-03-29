import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FolderListParams } from "@/services/fileService";
import { shareService } from "@/services/shareService";

const { apiDelete, apiGet, apiPatch, apiPost } = vi.hoisted(() => ({
	apiDelete: vi.fn(),
	apiGet: vi.fn(),
	apiPatch: vi.fn(),
	apiPost: vi.fn(),
}));

vi.mock("@/services/http", () => ({
	api: {
		delete: apiDelete,
		get: apiGet,
		patch: apiPatch,
		post: apiPost,
	},
}));

describe("shareService", () => {
	beforeEach(() => {
		apiDelete.mockReset();
		apiGet.mockReset();
		apiPatch.mockReset();
		apiPost.mockReset();
	});

	it("uses the expected authenticated share routes", () => {
		const createPayload = {
			file_id: 7,
			password: "secret",
			max_downloads: 3,
		};

		shareService.create(createPayload);
		shareService.listMine({ limit: 20, offset: 40 });
		shareService.update(7, {
			password: "updated-secret",
			expires_at: "2026-03-31T12:00:00Z",
			max_downloads: 9,
		});
		shareService.delete(7);
		shareService.batchDelete([7, 8]);

		expect(apiPost).toHaveBeenCalledWith("/shares", createPayload);
		expect(apiGet).toHaveBeenCalledWith("/shares", {
			params: { limit: 20, offset: 40 },
		});
		expect(apiPatch).toHaveBeenCalledWith("/shares/7", {
			password: "updated-secret",
			expires_at: "2026-03-31T12:00:00Z",
			max_downloads: 9,
		});
		expect(apiDelete).toHaveBeenCalledWith("/shares/7");
		expect(apiPost).toHaveBeenNthCalledWith(2, "/shares/batch-delete", {
			share_ids: [7, 8],
		});
	});

	it("uses the expected public share routes and download helpers", () => {
		const params: FolderListParams = {
			file_limit: 50,
			sort_by: "updated_at",
		};

		shareService.getInfo("token-1");
		shareService.verifyPassword("token-1", "secret");
		shareService.listContent("token-1", params);
		shareService.listSubfolderContent("token-1", 42, params);

		expect(apiGet).toHaveBeenNthCalledWith(1, "/s/token-1");
		expect(apiPost).toHaveBeenCalledWith("/s/token-1/verify", {
			password: "secret",
		});
		expect(apiGet).toHaveBeenNthCalledWith(2, "/s/token-1/content", {
			params,
		});
		expect(apiGet).toHaveBeenNthCalledWith(3, "/s/token-1/folders/42/content", {
			params,
		});
		expect(shareService.downloadPath("token-1")).toBe("/s/token-1/download");
		expect(shareService.thumbnailPath("token-1")).toBe("/s/token-1/thumbnail");
		expect(shareService.downloadFolderPath("token-1", 42)).toBe(
			"/s/token-1/files/42/download",
		);
		expect(shareService.downloadUrl("token-1")).toBe(
			"/api/v1/s/token-1/download",
		);
		expect(shareService.downloadFolderFileUrl("token-1", 42)).toBe(
			"/api/v1/s/token-1/files/42/download",
		);
	});
});
