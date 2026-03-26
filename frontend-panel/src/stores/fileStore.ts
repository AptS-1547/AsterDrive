import { create } from "zustand";
import { STORAGE_KEYS } from "@/config/app";
import { batchService } from "@/services/batchService";
import type { FolderListParams } from "@/services/fileService";
import { fileService } from "@/services/fileService";
import { searchService } from "@/services/searchService";
import { useAuthStore } from "@/stores/authStore";
import type {
	BatchResult,
	FileInfo,
	FolderContents,
	FolderInfo,
} from "@/types/api";

interface BreadcrumbItem {
	id: number | null;
	name: string;
}

interface Clipboard {
	fileIds: number[];
	folderIds: number[];
	mode: "copy" | "cut";
}

export type ViewMode = "grid" | "list";
export type SortBy = "name" | "size" | "created_at" | "updated_at" | "type";
export type SortOrder = "asc" | "desc";

const FILE_PAGE_SIZE = 100;
const FOLDER_LIMIT = 1000;

function getStored<T extends string>(key: string, fallback: T): T {
	if (typeof window === "undefined") return fallback;
	return (localStorage.getItem(key) as T) || fallback;
}

interface FileState {
	// Navigation
	currentFolderId: number | null;
	breadcrumb: BreadcrumbItem[];

	// Data
	folders: FolderInfo[];
	files: FileInfo[];
	loading: boolean;
	error: string | null;

	// Pagination
	filesTotalCount: number;
	foldersTotalCount: number;
	loadingMore: boolean;
	nextFileCursor: FolderContents["next_file_cursor"];

	// Search
	searchQuery: string | null;
	searchFolders: FolderInfo[];
	searchFiles: FileInfo[];

	// View preferences (persisted)
	viewMode: ViewMode;
	sortBy: SortBy;
	sortOrder: SortOrder;

	// Selection
	selectedFileIds: Set<number>;
	selectedFolderIds: Set<number>;

	// Clipboard
	clipboard: Clipboard | null;

	// Navigation actions
	navigateTo: (
		folderId: number | null,
		folderName?: string,
		breadcrumbPath?: BreadcrumbItem[],
	) => Promise<void>;
	refresh: () => Promise<void>;

	// Pagination actions
	loadMoreFiles: () => Promise<void>;
	hasMoreFiles: () => boolean;

	// View actions
	setViewMode: (mode: ViewMode) => void;
	setSortBy: (sortBy: SortBy) => void;
	setSortOrder: (sortOrder: SortOrder) => void;

	// Selection actions
	toggleFileSelection: (id: number) => void;
	toggleFolderSelection: (id: number) => void;
	selectAll: () => void;
	clearSelection: () => void;
	selectionCount: () => number;

	// Search actions
	search: (query: string) => Promise<void>;
	clearSearch: () => void;

	// Clipboard actions
	clipboardCopy: () => number;
	clipboardCut: () => number;
	clipboardPaste: () => Promise<{ mode: "copy" | "cut"; result: BatchResult }>;
	clearClipboard: () => void;

	// CRUD actions
	createFile: (name: string) => Promise<void>;
	createFolder: (name: string) => Promise<void>;
	deleteFile: (id: number) => Promise<void>;
	deleteFolder: (id: number) => Promise<void>;
	moveToFolder: (
		fileIds: number[],
		folderIds: number[],
		targetFolderId: number | null,
	) => Promise<BatchResult>;
}

export type { BreadcrumbItem, Clipboard };

function getInitialPageParams(
	sortBy: SortBy,
	sortOrder: SortOrder,
): FolderListParams {
	return {
		folder_limit: FOLDER_LIMIT,
		file_limit: FILE_PAGE_SIZE,
		sort_by: sortBy,
		sort_order: sortOrder,
	};
}

async function fetchFolder(folderId: number | null, params?: FolderListParams) {
	return folderId === null
		? await fileService.listRoot(params)
		: await fileService.listFolder(folderId, params);
}

async function resolveBreadcrumb(
	folderId: number | null,
	breadcrumbPath?: BreadcrumbItem[],
): Promise<BreadcrumbItem[]> {
	if (folderId === null) {
		return [{ id: null, name: "Root" }];
	}

	if (breadcrumbPath && breadcrumbPath.length > 0) {
		return breadcrumbPath;
	}

	const ancestors = await fileService.getFolderAncestors(folderId);
	return [
		{ id: null, name: "Root" },
		...ancestors.map((item) => ({ id: item.id, name: item.name })),
	];
}

export const useFileStore = create<FileState>((set, get) => ({
	currentFolderId: null,
	folders: [],
	files: [],
	breadcrumb: [{ id: null, name: "Root" }],
	loading: false,
	error: null,

	filesTotalCount: 0,
	foldersTotalCount: 0,
	loadingMore: false,
	nextFileCursor: null,

	viewMode: getStored(STORAGE_KEYS.viewMode, "list"),
	sortBy: getStored(STORAGE_KEYS.sortBy, "name"),
	sortOrder: getStored(STORAGE_KEYS.sortOrder, "asc"),

	searchQuery: null,
	searchFolders: [],
	searchFiles: [],

	selectedFileIds: new Set(),
	selectedFolderIds: new Set(),

	clipboard: null,

	navigateTo: async (folderId, folderName, breadcrumbPath) => {
		set({
			loading: true,
			error: null,
			searchQuery: null,
			searchFiles: [],
			searchFolders: [],
			selectedFileIds: new Set(),
			selectedFolderIds: new Set(),
			files: [],
			folders: [],
			filesTotalCount: 0,
			foldersTotalCount: 0,
			nextFileCursor: null,
		});
		try {
			const [contents, newBreadcrumb] = await Promise.all([
				fetchFolder(folderId, getInitialPageParams(get().sortBy, get().sortOrder)),
				resolveBreadcrumb(folderId, breadcrumbPath),
			]);

			set({
				currentFolderId: folderId,
				folders: contents.folders,
				files: contents.files,
				foldersTotalCount: contents.folders_total,
				filesTotalCount: contents.files_total,
				nextFileCursor: contents.next_file_cursor ?? null,
				breadcrumb: newBreadcrumb,
				loading: false,
				error: null,
			});
		} catch (error) {
			const msg =
				error && typeof error === "object" && "message" in error
					? (error as { message: string }).message
					: folderName || "Failed to load folder";
			set({ loading: false, error: msg });
			throw error;
		}
	},

	refresh: async () => {
		const { currentFolderId } = get();
		set({
			loading: true,
			files: [],
			folders: [],
			filesTotalCount: 0,
			foldersTotalCount: 0,
			nextFileCursor: null,
		});
		try {
			const contents = await fetchFolder(
				currentFolderId,
				getInitialPageParams(get().sortBy, get().sortOrder),
			);
			set({
				folders: contents.folders,
				files: contents.files,
				foldersTotalCount: contents.folders_total,
				filesTotalCount: contents.files_total,
				nextFileCursor: contents.next_file_cursor ?? null,
				loading: false,
			});
		} catch (error) {
			set({ loading: false });
			throw error;
		}
	},

	loadMoreFiles: async () => {
		const { currentFolderId, nextFileCursor, loadingMore, sortBy, sortOrder } =
			get();
		if (loadingMore || !nextFileCursor) return;

		set({ loadingMore: true });
		try {
			const contents = await fetchFolder(currentFolderId, {
				folder_limit: 0,
				file_limit: FILE_PAGE_SIZE,
				file_after_value: nextFileCursor.value,
				file_after_id: nextFileCursor.id,
				sort_by: sortBy,
				sort_order: sortOrder,
			});
			set((state) => ({
				files: [...state.files, ...contents.files],
				nextFileCursor: contents.next_file_cursor ?? null,
				loadingMore: false,
			}));
		} catch {
			set({ loadingMore: false });
		}
	},

	hasMoreFiles: () => {
		const cursor = get().nextFileCursor;
		return cursor !== null && cursor !== undefined;
	},

	setViewMode: (mode) => {
		localStorage.setItem(STORAGE_KEYS.viewMode, mode);
		set({ viewMode: mode });
	},

	setSortBy: (sortBy) => {
		localStorage.setItem(STORAGE_KEYS.sortBy, sortBy);
		set({ sortBy, files: [], folders: [], nextFileCursor: null, filesTotalCount: 0, foldersTotalCount: 0 });
		const { currentFolderId, sortOrder } = get();
		void fetchFolder(
			currentFolderId,
			getInitialPageParams(sortBy, sortOrder),
		).then((contents) =>
			set({
				files: contents.files,
				folders: contents.folders,
				filesTotalCount: contents.files_total,
				foldersTotalCount: contents.folders_total,
				nextFileCursor: contents.next_file_cursor ?? null,
			}),
		);
	},

	setSortOrder: (sortOrder) => {
		localStorage.setItem(STORAGE_KEYS.sortOrder, sortOrder);
		set({ sortOrder, files: [], folders: [], nextFileCursor: null, filesTotalCount: 0, foldersTotalCount: 0 });
		const { currentFolderId, sortBy } = get();
		void fetchFolder(
			currentFolderId,
			getInitialPageParams(sortBy, sortOrder),
		).then((contents) =>
			set({
				files: contents.files,
				folders: contents.folders,
				filesTotalCount: contents.files_total,
				foldersTotalCount: contents.folders_total,
				nextFileCursor: contents.next_file_cursor ?? null,
			}),
		);
	},

	toggleFileSelection: (id) => {
		const next = new Set(get().selectedFileIds);
		if (next.has(id)) {
			next.delete(id);
		} else {
			next.add(id);
		}
		set({ selectedFileIds: next });
	},

	toggleFolderSelection: (id) => {
		const next = new Set(get().selectedFolderIds);
		if (next.has(id)) {
			next.delete(id);
		} else {
			next.add(id);
		}
		set({ selectedFolderIds: next });
	},

	selectAll: () => {
		const { files, folders } = get();
		set({
			selectedFileIds: new Set(files.map((f) => f.id)),
			selectedFolderIds: new Set(folders.map((f) => f.id)),
		});
	},

	clearSelection: () => {
		set({ selectedFileIds: new Set(), selectedFolderIds: new Set() });
	},

	selectionCount: () => {
		const { selectedFileIds, selectedFolderIds } = get();
		return selectedFileIds.size + selectedFolderIds.size;
	},

	search: async (query) => {
		set({ loading: true, searchQuery: query });
		try {
			const results = await searchService.search({ q: query, limit: 100 });
			set({
				searchFiles: results.files as FileInfo[],
				searchFolders: results.folders,
				loading: false,
				selectedFileIds: new Set(),
				selectedFolderIds: new Set(),
			});
		} catch (error) {
			set({ loading: false });
			throw error;
		}
	},

	clearSearch: () => {
		set({
			searchQuery: null,
			searchFiles: [],
			searchFolders: [],
		});
	},

	clipboardCopy: () => {
		const { selectedFileIds, selectedFolderIds } = get();
		const count = selectedFileIds.size + selectedFolderIds.size;
		if (count === 0) return 0;
		set({
			clipboard: {
				fileIds: Array.from(selectedFileIds),
				folderIds: Array.from(selectedFolderIds),
				mode: "copy",
			},
		});
		return count;
	},

	clipboardCut: () => {
		const { selectedFileIds, selectedFolderIds } = get();
		const count = selectedFileIds.size + selectedFolderIds.size;
		if (count === 0) return 0;
		set({
			clipboard: {
				fileIds: Array.from(selectedFileIds),
				folderIds: Array.from(selectedFolderIds),
				mode: "cut",
			},
		});
		return count;
	},

	clipboardPaste: async () => {
		const { clipboard, currentFolderId } = get();
		if (!clipboard) throw new Error("No clipboard");

		const result =
			clipboard.mode === "copy"
				? await batchService.batchCopy(
						clipboard.fileIds,
						clipboard.folderIds,
						currentFolderId,
					)
				: await batchService.batchMove(
						clipboard.fileIds,
						clipboard.folderIds,
						currentFolderId,
					);

		const mode = clipboard.mode;

		// Cut: clear clipboard after paste; Copy: keep clipboard
		if (mode === "cut") {
			set({ clipboard: null });
		}

		// Silent refresh
		get().clearSelection();
		const contents = await fetchFolder(
			currentFolderId,
			getInitialPageParams(get().sortBy, get().sortOrder),
		);
		set({
			folders: contents.folders,
			files: contents.files,
			foldersTotalCount: contents.folders_total,
			filesTotalCount: contents.files_total,
		});

		return { mode, result };
	},

	clearClipboard: () => {
		set({ clipboard: null });
	},

	createFile: async (name) => {
		const { currentFolderId } = get();
		await fileService.createEmptyFile(name, currentFolderId);
		await Promise.all([get().refresh(), useAuthStore.getState().refreshUser()]);
	},

	createFolder: async (name) => {
		const { currentFolderId } = get();
		await fileService.createFolder(name, currentFolderId);
		await get().refresh();
	},

	deleteFile: async (id) => {
		await fileService.deleteFile(id);
		// Remove from selection
		const next = new Set(get().selectedFileIds);
		next.delete(id);
		set({ selectedFileIds: next });
		await Promise.all([get().refresh(), useAuthStore.getState().refreshUser()]);
	},

	deleteFolder: async (id) => {
		await fileService.deleteFolder(id);
		const next = new Set(get().selectedFolderIds);
		next.delete(id);
		set({ selectedFolderIds: next });
		await Promise.all([get().refresh(), useAuthStore.getState().refreshUser()]);
	},

	moveToFolder: async (fileIds, folderIds, targetFolderId) => {
		const result = await batchService.batchMove(
			fileIds,
			folderIds,
			targetFolderId,
		);
		get().clearSelection();
		// Silent refresh — don't set loading to avoid flash
		const { currentFolderId } = get();
		const contents = await fetchFolder(
			currentFolderId,
			getInitialPageParams(get().sortBy, get().sortOrder),
		);
		set({
			folders: contents.folders,
			files: contents.files,
			foldersTotalCount: contents.folders_total,
			filesTotalCount: contents.files_total,
		});
		return result;
	},
}));
