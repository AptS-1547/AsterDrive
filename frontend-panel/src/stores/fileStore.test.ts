import { waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FILE_PAGE_SIZE, FOLDER_LIMIT } from "@/lib/constants";
import { createFolderContents } from "@/test/fixtures";

const listRoot = vi.fn();
const listFolder = vi.fn();
const getFolderAncestors = vi.fn();
const queuePreferenceSync = vi.fn();

vi.mock("@/services/fileService", () => ({
	fileService: {
		listRoot,
		listFolder,
		getFolderAncestors,
	},
}));

vi.mock("@/lib/preferenceSync", () => ({
	cancelPreferenceSync: vi.fn(),
	queuePreferenceSync,
}));

vi.mock("@/services/batchService", () => ({
	batchService: {
		batchCopy: vi.fn(),
		batchDelete: vi.fn(),
		batchMove: vi.fn(),
	},
}));

vi.mock("@/stores/authStore", () => ({
	useAuthStore: {
		getState: () => ({
			refreshUser: vi.fn().mockResolvedValue(undefined),
		}),
	},
}));

async function loadStore() {
	vi.resetModules();
	const { useFileStore } = await import("@/stores/fileStore");
	return useFileStore;
}

function expectAbortOptions(options: unknown) {
	expect(options).toEqual(
		expect.objectContaining({
			signal: expect.any(AbortSignal),
		}),
	);
}

function createDeferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason?: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

function createCanceledError() {
	const error = new Error("canceled");
	error.name = "CanceledError";
	return Object.assign(error, { code: "ERR_CANCELED" });
}

describe("useFileStore", () => {
	beforeEach(() => {
		listRoot.mockReset();
		listFolder.mockReset();
		getFolderAncestors.mockReset();
		queuePreferenceSync.mockReset();
	});

	it("loads the root folder with initial sort and page params", async () => {
		listRoot.mockResolvedValue(
			createFolderContents({
				files: [{ id: 101, name: "report.txt" }],
				folders: [{ id: 7, name: "Docs" }],
				files_total: 1,
				folders_total: 1,
			}),
		);

		const useFileStore = await loadStore();

		await useFileStore.getState().navigateTo(null);

		expect(listRoot).toHaveBeenCalledTimes(1);
		expect(listRoot).toHaveBeenCalledWith(
			{
				folder_limit: FOLDER_LIMIT,
				file_limit: FILE_PAGE_SIZE,
				sort_by: "name",
				sort_order: "asc",
			},
			expect.any(Object),
		);
		expectAbortOptions(listRoot.mock.calls[0][1]);
		expect(useFileStore.getState()).toMatchObject({
			currentFolderId: null,
			breadcrumb: [{ id: null, name: "Root" }],
			filesTotalCount: 1,
			foldersTotalCount: 1,
		});
		expect(useFileStore.getState().files).toEqual([
			expect.objectContaining({ id: 101, name: "report.txt" }),
		]);
	});

	it("appends the next cursor page when loading more files", async () => {
		listRoot
			.mockResolvedValueOnce(
				createFolderContents({
					files: [{ id: 101, name: "alpha.txt" }],
					files_total: 2,
					next_file_cursor: { id: 101, value: "alpha.txt" },
				}),
			)
			.mockResolvedValueOnce(
				createFolderContents({
					files: [{ id: 102, name: "beta.txt" }],
					files_total: 2,
					next_file_cursor: null,
				}),
			);

		const useFileStore = await loadStore();

		await useFileStore.getState().navigateTo(null);
		await useFileStore.getState().loadMoreFiles();

		expect(listRoot).toHaveBeenNthCalledWith(
			2,
			{
				folder_limit: 0,
				file_limit: FILE_PAGE_SIZE,
				file_after_value: "alpha.txt",
				file_after_id: 101,
				sort_by: "name",
				sort_order: "asc",
			},
			expect.any(Object),
		);
		expectAbortOptions(listRoot.mock.calls[1][1]);
		expect(useFileStore.getState().files).toEqual([
			expect.objectContaining({ id: 101 }),
			expect.objectContaining({ id: 102 }),
		]);
		expect(useFileStore.getState().nextFileCursor).toBeNull();
	});

	it("refetches with the updated sort field and persists the preference", async () => {
		listRoot
			.mockResolvedValueOnce(
				createFolderContents({
					files: [{ id: 101, name: "alpha.txt" }],
				}),
			)
			.mockResolvedValueOnce(
				createFolderContents({
					files: [{ id: 102, name: "latest.txt" }],
				}),
			);

		const useFileStore = await loadStore();

		await useFileStore.getState().navigateTo(null);
		useFileStore.getState().setSortBy("updated_at");

		await waitFor(() => {
			expect(listRoot).toHaveBeenNthCalledWith(
				2,
				{
					folder_limit: FOLDER_LIMIT,
					file_limit: FILE_PAGE_SIZE,
					sort_by: "updated_at",
					sort_order: "asc",
				},
				expect.any(Object),
			);
		});
		expectAbortOptions(listRoot.mock.calls[1][1]);

		expect(queuePreferenceSync).toHaveBeenCalledWith({
			sort_by: "updated_at",
		});
		expect(localStorage.getItem("aster-sort-by")).toBe("updated_at");
		expect(useFileStore.getState().files).toEqual([
			expect.objectContaining({ id: 102, name: "latest.txt" }),
		]);
	});

	it("aborts the previous folder request when navigation changes", async () => {
		const first = createDeferred<ReturnType<typeof createFolderContents>>();
		const second = createDeferred<ReturnType<typeof createFolderContents>>();

		listFolder
			.mockImplementationOnce((_id, _params, options) => {
				const signal = options?.signal as AbortSignal;
				signal.addEventListener(
					"abort",
					() => {
						first.reject(createCanceledError());
					},
					{ once: true },
				);
				return first.promise;
			})
			.mockImplementationOnce(() => second.promise);

		const useFileStore = await loadStore();

		const firstNavigate = useFileStore.getState().navigateTo(1, undefined, [
			{ id: null, name: "Root" },
			{ id: 1, name: "Docs" },
		]);
		const firstSignal = listFolder.mock.calls[0][2]?.signal as AbortSignal;

		const secondNavigate = useFileStore.getState().navigateTo(2, undefined, [
			{ id: null, name: "Root" },
			{ id: 2, name: "Media" },
		]);

		expect(firstSignal.aborted).toBe(true);

		second.resolve(
			createFolderContents({
				files: [{ id: 202, name: "cover.png" }],
				files_total: 1,
			}),
		);

		await expect(firstNavigate).resolves.toBeUndefined();
		await expect(secondNavigate).resolves.toBeUndefined();
		expect(useFileStore.getState().currentFolderId).toBe(2);
		expect(useFileStore.getState().files).toEqual([
			expect.objectContaining({ id: 202, name: "cover.png" }),
		]);
	});

	it("ignores stale folder results that resolve after a newer navigation", async () => {
		const first = createDeferred<ReturnType<typeof createFolderContents>>();
		const second = createDeferred<ReturnType<typeof createFolderContents>>();

		listFolder
			.mockImplementationOnce(() => first.promise)
			.mockImplementationOnce(() => second.promise);

		const useFileStore = await loadStore();

		const firstNavigate = useFileStore.getState().navigateTo(1, undefined, [
			{ id: null, name: "Root" },
			{ id: 1, name: "Docs" },
		]);
		const secondNavigate = useFileStore.getState().navigateTo(2, undefined, [
			{ id: null, name: "Root" },
			{ id: 2, name: "Media" },
		]);

		second.resolve(
			createFolderContents({
				files: [{ id: 202, name: "cover.png" }],
				files_total: 1,
			}),
		);
		await secondNavigate;

		first.resolve(
			createFolderContents({
				files: [{ id: 101, name: "stale.txt" }],
				files_total: 1,
			}),
		);
		await firstNavigate;

		expect(useFileStore.getState().currentFolderId).toBe(2);
		expect(useFileStore.getState().breadcrumb).toEqual([
			{ id: null, name: "Root" },
			{ id: 2, name: "Media" },
		]);
		expect(useFileStore.getState().files).toEqual([
			expect.objectContaining({ id: 202, name: "cover.png" }),
		]);
	});
});
