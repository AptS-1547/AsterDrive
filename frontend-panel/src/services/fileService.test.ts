import { beforeEach, describe, expect, it, vi } from "vitest";
import { PERSONAL_WORKSPACE } from "@/lib/workspace";

const mockState = vi.hoisted(() => {
	class MockApiError extends Error {
		code: number;

		constructor(code: number, message: string) {
			super(message);
			this.code = code;
		}
	}

	return {
		ApiError: MockApiError,
		clientPut: vi.fn(),
		delete: vi.fn(),
		get: vi.fn(),
		patch: vi.fn(),
		post: vi.fn(),
	};
});

vi.mock("@/services/http", () => ({
	ApiError: mockState.ApiError,
	api: {
		client: {
			put: mockState.clientPut,
		},
		delete: mockState.delete,
		get: mockState.get,
		patch: mockState.patch,
		post: mockState.post,
	},
}));

describe("fileService", () => {
	beforeEach(async () => {
		mockState.clientPut.mockReset();
		mockState.delete.mockReset();
		mockState.get.mockReset();
		mockState.patch.mockReset();
		mockState.post.mockReset();
		const { setPublicSiteUrl } = await import("@/lib/publicSiteUrl");
		setPublicSiteUrl(null);
	});

	it("uses the expected file and folder endpoints", async () => {
		const { createFileService, fileService } = await import(
			"@/services/fileService"
		);

		fileService.listRoot({ file_limit: 50 });
		fileService.listFolder(7, { sort_by: "updated_at" });
		fileService.getFolderAncestors(7);
		fileService.getFolderInfo(7);
		fileService.createFolder("Docs", null);
		fileService.renameFolder(7, "Renamed");
		fileService.getFile(8);
		fileService.getDirectLinkToken(8);
		fileService.createWopiSession(8, "custom.onlyoffice");
		fileService.deleteFile(8);
		fileService.renameFile(8, "notes.md");
		fileService.setFileLock(8, true);
		fileService.setFolderLock(7, false);
		fileService.createEmptyFile("draft.md", 7);
		fileService.copyFile(8, null);
		fileService.copyFolder(7, 3);
		fileService.listVersions(8);
		fileService.restoreVersion(8, 2);
		fileService.deleteVersion(8, 2);

		expect(mockState.get).toHaveBeenNthCalledWith(1, "/folders", {
			params: { file_limit: 50 },
		});
		expect(mockState.get).toHaveBeenNthCalledWith(2, "/folders/7", {
			params: { sort_by: "updated_at" },
		});
		expect(mockState.get).toHaveBeenNthCalledWith(3, "/folders/7/ancestors");
		expect(mockState.get).toHaveBeenNthCalledWith(4, "/folders/7/info");
		expect(mockState.post).toHaveBeenNthCalledWith(1, "/folders", {
			name: "Docs",
			parent_id: null,
		});
		expect(mockState.patch).toHaveBeenNthCalledWith(1, "/folders/7", {
			name: "Renamed",
		});
		expect(mockState.get).toHaveBeenNthCalledWith(5, "/files/8");
		expect(mockState.get).toHaveBeenNthCalledWith(6, "/files/8/direct-link");
		expect(mockState.post).toHaveBeenNthCalledWith(2, "/files/8/wopi/open", {
			app_key: "custom.onlyoffice",
		});
		expect(mockState.delete).toHaveBeenNthCalledWith(1, "/files/8");
		expect(mockState.patch).toHaveBeenNthCalledWith(2, "/files/8", {
			name: "notes.md",
		});
		expect(mockState.post).toHaveBeenNthCalledWith(3, "/files/8/lock", {
			locked: true,
		});
		expect(mockState.post).toHaveBeenNthCalledWith(4, "/folders/7/lock", {
			locked: false,
		});
		expect(mockState.post).toHaveBeenNthCalledWith(5, "/files/new", {
			name: "draft.md",
			folder_id: 7,
		});
		expect(mockState.post).toHaveBeenNthCalledWith(6, "/files/8/copy", {
			folder_id: null,
		});
		expect(mockState.post).toHaveBeenNthCalledWith(7, "/folders/7/copy", {
			parent_id: 3,
		});
		expect(mockState.get).toHaveBeenNthCalledWith(7, "/files/8/versions");
		expect(mockState.post).toHaveBeenNthCalledWith(
			8,
			"/files/8/versions/2/restore",
		);
		expect(mockState.delete).toHaveBeenNthCalledWith(2, "/files/8/versions/2");
		expect(fileService.downloadPath(8)).toBe("/files/8/download");
		expect(fileService.downloadUrl(8)).toBe("/api/v1/files/8/download");
		expect(fileService.directPath("token-1", "clip 1.mp4")).toBe(
			"/d/token-1/clip%201.mp4",
		);
		expect(fileService.directUrl("token-1", "clip 1.mp4")).toBe(
			new URL("/d/token-1/clip%201.mp4", window.location.origin).toString(),
		);
		expect(fileService.forceDownloadPath("token-1", "clip 1.mp4")).toBe(
			"/d/token-1/clip%201.mp4?download=1",
		);
		expect(fileService.forceDownloadUrl("token-1", "clip 1.mp4")).toBe(
			new URL(
				"/d/token-1/clip%201.mp4?download=1",
				window.location.origin,
			).toString(),
		);
		expect(fileService.thumbnailPath(8)).toBe("/files/8/thumbnail");

		const teamFileService = createFileService({ kind: "team", teamId: 9 });
		teamFileService.listRoot();
		teamFileService.getFile(8);
		teamFileService.getDirectLinkToken(8);
		teamFileService.listVersions(8);

		expect(mockState.get).toHaveBeenCalledWith("/teams/9/folders", {
			params: undefined,
		});
		expect(mockState.get).toHaveBeenCalledWith("/teams/9/files/8");
		expect(mockState.get).toHaveBeenCalledWith("/teams/9/files/8/direct-link");
		expect(mockState.get).toHaveBeenCalledWith("/teams/9/files/8/versions");
		expect(teamFileService.downloadPath(8)).toBe("/teams/9/files/8/download");
		expect(teamFileService.downloadUrl(8)).toBe(
			"/api/v1/teams/9/files/8/download",
		);
		expect(teamFileService.thumbnailPath(8)).toBe("/teams/9/files/8/thumbnail");
	});

	it("updates file content with optimistic concurrency headers", async () => {
		mockState.clientPut.mockResolvedValue({
			data: {
				data: { id: 8, name: "notes.md" },
			},
		});
		const { fileService } = await import("@/services/fileService");

		await expect(
			fileService.updateContent(8, "hello", "etag-1"),
		).resolves.toEqual({
			id: 8,
			name: "notes.md",
		});
		expect(mockState.clientPut).toHaveBeenCalledWith(
			"/files/8/content",
			"hello",
			{
				headers: {
					"Content-Type": "application/octet-stream",
					"If-Match": "etag-1",
				},
			},
		);
	});

	it("wraps axios-like update failures in ApiError and preserves the status", async () => {
		mockState.clientPut.mockRejectedValue({
			response: {
				status: 412,
				data: {
					code: 2003,
					msg: "conflict",
				},
			},
		});
		const { fileService } = await import("@/services/fileService");

		await expect(fileService.updateContent(8, "hello")).rejects.toEqual(
			expect.objectContaining({
				code: 2003,
				message: "conflict",
				status: 412,
			}),
		);
	});

	it("rethrows unknown update failures as-is", async () => {
		const failure = new Error("network boom");
		mockState.clientPut.mockRejectedValue(failure);
		const { fileService } = await import("@/services/fileService");

		await expect(fileService.updateContent(8, "hello")).rejects.toBe(failure);
	});

	it("rethrows axios-like update failures without a response as-is", async () => {
		const failure = { response: undefined, message: "network boom" };
		mockState.clientPut.mockRejectedValue(failure);
		const { fileService } = await import("@/services/fileService");

		await expect(fileService.updateContent(8, "hello")).rejects.toBe(failure);
	});

	it("normalizes trailing slashes when building download URLs", async () => {
		vi.resetModules();
		vi.doMock("@/config/app", () => ({
			config: {
				apiBaseUrl: "/api/v1///",
				appName: "AsterDrive",
				appVersion: "test",
			},
		}));

		const { createFileService } = await import("@/services/fileService");
		expect(createFileService(PERSONAL_WORKSPACE).downloadUrl(8)).toBe(
			"/api/v1/files/8/download",
		);
		expect(createFileService({ kind: "team", teamId: 9 }).downloadUrl(8)).toBe(
			"/api/v1/teams/9/files/8/download",
		);

		vi.doUnmock("@/config/app");
	});

	it("uses the configured public site URL for direct absolute links", async () => {
		const { setPublicSiteUrl } = await import("@/lib/publicSiteUrl");
		const { fileService } = await import("@/services/fileService");

		setPublicSiteUrl("https://drive.example.com");

		expect(fileService.directUrl("token-1", "clip 1.mp4")).toBe(
			"https://drive.example.com/d/token-1/clip%201.mp4",
		);
		expect(fileService.forceDownloadUrl("token-1", "clip 1.mp4")).toBe(
			"https://drive.example.com/d/token-1/clip%201.mp4?download=1",
		);

		setPublicSiteUrl(null);
	});
});
