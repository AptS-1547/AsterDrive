import { beforeEach, describe, expect, it, vi } from "vitest";
import { STORAGE_KEYS } from "@/config/app";

const mockState = vi.hoisted(() => ({
	batchCopy: vi.fn(),
	batchMove: vi.fn(),
	getFolderAncestors: vi.fn(),
	listFolder: vi.fn(),
	listRoot: vi.fn(),
	queuePreferenceSync: vi.fn(),
	refreshUser: vi.fn(async () => undefined),
	search: vi.fn(),
}));

vi.mock("@/services/fileService", () => ({
	fileService: {
		getFolderAncestors: mockState.getFolderAncestors,
		listFolder: mockState.listFolder,
		listRoot: mockState.listRoot,
	},
}));

vi.mock("@/lib/preferenceSync", () => ({
	cancelPreferenceSync: vi.fn(),
	queuePreferenceSync: mockState.queuePreferenceSync,
}));

vi.mock("@/services/batchService", () => ({
	batchService: {
		batchCopy: mockState.batchCopy,
		batchMove: mockState.batchMove,
	},
}));

vi.mock("@/services/searchService", () => ({
	searchService: {
		search: mockState.search,
	},
}));

vi.mock("@/stores/authStore", () => ({
	useAuthStore: {
		getState: () => ({
			refreshUser: mockState.refreshUser,
		}),
	},
}));

async function loadStore() {
	vi.resetModules();
	return await import("@/stores/fileStore");
}

function createContents() {
	return {
		files: [],
		folders: [],
		files_total: 0,
		folders_total: 0,
		next_file_cursor: null,
	};
}

describe("useFileStore edge cases", () => {
	beforeEach(() => {
		localStorage.clear();
		mockState.batchCopy.mockReset();
		mockState.batchMove.mockReset();
		mockState.getFolderAncestors.mockReset();
		mockState.listFolder.mockReset();
		mockState.listRoot.mockReset();
		mockState.queuePreferenceSync.mockReset();
		mockState.refreshUser.mockReset();
		mockState.refreshUser.mockResolvedValue(undefined);
		mockState.search.mockReset();
	});

	it("persists view mode changes", async () => {
		const { useFileStore } = await loadStore();

		useFileStore.getState().setViewMode("grid");

		expect(useFileStore.getState().viewMode).toBe("grid");
		expect(localStorage.getItem("aster-view-mode")).toBe("grid");
		expect(mockState.queuePreferenceSync).toHaveBeenCalledWith({
			view_mode: "grid",
		});
	});

	it("persists browser open mode changes and can select a single folder", async () => {
		const { useFileStore } = await loadStore();

		useFileStore.getState().setBrowserOpenMode("double_click");
		useFileStore.getState().selectOnlyFolder(8);

		expect(useFileStore.getState().browserOpenMode).toBe("double_click");
		expect(localStorage.getItem("aster-browser-open-mode")).toBe(
			"double_click",
		);
		expect(mockState.queuePreferenceSync).toHaveBeenCalledWith({
			browser_open_mode: "double_click",
		});
		expect(useFileStore.getState().selectedFileIds.size).toBe(0);
		expect(useFileStore.getState().selectedFolderIds).toEqual(new Set([8]));
	});

	it("falls back to default preferences when stored values are invalid", async () => {
		localStorage.setItem(STORAGE_KEYS.viewMode, "tiles");
		localStorage.setItem(STORAGE_KEYS.browserOpenMode, "hover");
		localStorage.setItem(STORAGE_KEYS.sortBy, "owner");
		localStorage.setItem(STORAGE_KEYS.sortOrder, "random");

		const { useFileStore } = await loadStore();

		expect(useFileStore.getState()).toMatchObject({
			viewMode: "list",
			browserOpenMode: "single_click",
			sortBy: "name",
			sortOrder: "asc",
		});
	});

	it("falls back to default preferences when localStorage reads fail", async () => {
		const getItem = vi
			.spyOn(Storage.prototype, "getItem")
			.mockImplementation(() => {
				throw new Error("storage blocked");
			});

		try {
			const { useFileStore } = await loadStore();

			expect(useFileStore.getState()).toMatchObject({
				viewMode: "list",
				browserOpenMode: "single_click",
				sortBy: "name",
				sortOrder: "asc",
			});
		} finally {
			getItem.mockRestore();
		}
	});

	it("keeps preference updates in memory when localStorage writes fail", async () => {
		const setItem = vi
			.spyOn(Storage.prototype, "setItem")
			.mockImplementation(() => {
				throw new Error("quota exceeded");
			});

		try {
			const { useFileStore } = await loadStore();

			expect(() => useFileStore.getState().setViewMode("grid")).not.toThrow();

			expect(useFileStore.getState().viewMode).toBe("grid");
			expect(mockState.queuePreferenceSync).toHaveBeenCalledWith({
				view_mode: "grid",
			});
		} finally {
			setItem.mockRestore();
		}
	});

	it("populates and clears search results", async () => {
		mockState.search.mockResolvedValue({
			files: [{ id: 7, name: "notes.txt" }],
			folders: [{ id: 8, name: "Docs" }],
		});
		const { useFileStore } = await loadStore();
		useFileStore.setState({
			selectedFileIds: new Set([7]),
			selectedFolderIds: new Set([8]),
		});

		await useFileStore.getState().search("notes");

		expect(mockState.search).toHaveBeenCalledWith(
			{
				q: "notes",
				limit: 100,
			},
			expect.objectContaining({
				signal: expect.any(AbortSignal),
			}),
		);
		expect(useFileStore.getState()).toMatchObject({
			loading: false,
			searchQuery: "notes",
		});
		expect(useFileStore.getState().searchFiles).toEqual([
			expect.objectContaining({ id: 7 }),
		]);
		expect(useFileStore.getState().searchFolders).toEqual([
			expect.objectContaining({ id: 8 }),
		]);
		expect(useFileStore.getState().selectedFileIds.size).toBe(0);
		expect(useFileStore.getState().selectedFolderIds.size).toBe(0);

		useFileStore.getState().clearSearch();

		expect(useFileStore.getState()).toMatchObject({
			searchQuery: null,
			searchFiles: [],
			searchFolders: [],
		});
	});

	it("tracks clipboard state for copy and cut selections", async () => {
		const { useFileStore } = await loadStore();
		useFileStore.setState({
			selectedFileIds: new Set([1, 2]),
			selectedFolderIds: new Set([3]),
		});

		expect(useFileStore.getState().clipboardCopy()).toBe(3);
		expect(useFileStore.getState().clipboard).toEqual({
			fileIds: [1, 2],
			folderIds: [3],
			mode: "copy",
		});

		expect(useFileStore.getState().clipboardCut()).toBe(3);
		expect(useFileStore.getState().clipboard).toEqual({
			fileIds: [1, 2],
			folderIds: [3],
			mode: "cut",
		});
	});

	it("pastes clipboard contents and clears cut-mode clipboard", async () => {
		mockState.listRoot.mockResolvedValue(createContents());
		mockState.batchMove.mockResolvedValue({
			succeeded: 2,
			failed: 0,
			errors: [],
		});
		const { useFileStore } = await loadStore();
		useFileStore.setState({
			clipboard: {
				fileIds: [1],
				folderIds: [2],
				mode: "cut",
			},
			selectedFileIds: new Set([1]),
			selectedFolderIds: new Set([2]),
		});

		await expect(useFileStore.getState().clipboardPaste()).resolves.toEqual({
			mode: "cut",
			result: {
				succeeded: 2,
				failed: 0,
				errors: [],
			},
		});

		expect(mockState.batchMove).toHaveBeenCalledWith([1], [2], null);
		expect(useFileStore.getState().clipboard).toBeNull();
		expect(useFileStore.getState().selectedFileIds.size).toBe(0);
		expect(useFileStore.getState().selectedFolderIds.size).toBe(0);
	});
});
