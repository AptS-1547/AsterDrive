import { create } from "zustand";
import { STORAGE_KEYS } from "@/config/app";
import { batchService } from "@/services/batchService";
import type { FolderListParams } from "@/services/fileService";
import { fileService } from "@/services/fileService";
import { searchService } from "@/services/searchService";
import { useAuthStore } from "@/stores/authStore";
import type { BatchResult, FileInfo, FolderInfo } from "@/types/api";

interface BreadcrumbItem {
	id: number | null;
	name: string;
}

type ViewMode = "grid" | "list";
type SortBy = "name" | "date" | "size" | "type";
type SortOrder = "asc" | "desc";

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

	// Navigation actions
	navigateTo: (folderId: number | null, folderName?: string) => Promise<void>;
	refresh: () => Promise<void>;

	// Pagination actions
	loadMoreFiles: () => Promise<void>;
	hasMoreFiles: () => boolean;

	// View actions
	setViewMode: (mode: ViewMode) => void;
	setSortBy: (sortBy: SortBy) => void;
	toggleSortOrder: () => void;

	// Selection actions
	toggleFileSelection: (id: number) => void;
	toggleFolderSelection: (id: number) => void;
	selectAll: () => void;
	clearSelection: () => void;
	selectionCount: () => number;

	// Search actions
	search: (query: string) => Promise<void>;
	clearSearch: () => void;

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

export type { BreadcrumbItem, SortBy, SortOrder, ViewMode };

const initialPageParams: FolderListParams = {
	folder_limit: FOLDER_LIMIT,
	file_limit: FILE_PAGE_SIZE,
};

async function fetchFolder(folderId: number | null, params?: FolderListParams) {
	return folderId === null
		? await fileService.listRoot(params)
		: await fileService.listFolder(folderId, params);
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

	viewMode: getStored(STORAGE_KEYS.viewMode, "list"),
	sortBy: getStored(STORAGE_KEYS.sortBy, "name"),
	sortOrder: getStored(STORAGE_KEYS.sortOrder, "asc"),

	searchQuery: null,
	searchFolders: [],
	searchFiles: [],

	selectedFileIds: new Set(),
	selectedFolderIds: new Set(),

	navigateTo: async (folderId, folderName) => {
		set({
			loading: true,
			error: null,
			searchQuery: null,
			searchFiles: [],
			searchFolders: [],
			selectedFileIds: new Set(),
			selectedFolderIds: new Set(),
		});
		try {
			const contents = await fetchFolder(folderId, initialPageParams);

			// Update breadcrumb
			const { breadcrumb } = get();
			let newBreadcrumb: BreadcrumbItem[];

			if (folderId === null) {
				newBreadcrumb = [{ id: null, name: "Root" }];
			} else {
				const existingIndex = breadcrumb.findIndex((b) => b.id === folderId);
				if (existingIndex >= 0) {
					// Going back up -- trim breadcrumb
					newBreadcrumb = breadcrumb.slice(0, existingIndex + 1);
				} else {
					// Going deeper
					newBreadcrumb = [
						...breadcrumb,
						{
							id: folderId,
							name: folderName || `Folder ${folderId}`,
						},
					];
				}
			}

			set({
				currentFolderId: folderId,
				folders: contents.folders,
				files: contents.files,
				foldersTotalCount: contents.folders_total,
				filesTotalCount: contents.files_total,
				breadcrumb: newBreadcrumb,
				loading: false,
				error: null,
			});
		} catch (error) {
			const msg =
				error && typeof error === "object" && "message" in error
					? (error as { message: string }).message
					: "Failed to load folder";
			set({ loading: false, error: msg });
			throw error;
		}
	},

	refresh: async () => {
		const { currentFolderId } = get();
		set({ loading: true });
		try {
			const contents = await fetchFolder(currentFolderId, initialPageParams);
			set({
				folders: contents.folders,
				files: contents.files,
				foldersTotalCount: contents.folders_total,
				filesTotalCount: contents.files_total,
				loading: false,
			});
		} catch (error) {
			set({ loading: false });
			throw error;
		}
	},

	loadMoreFiles: async () => {
		const { currentFolderId, files, filesTotalCount, loadingMore } = get();
		if (loadingMore || files.length >= filesTotalCount) return;

		set({ loadingMore: true });
		try {
			const contents = await fetchFolder(currentFolderId, {
				folder_limit: 0,
				file_limit: FILE_PAGE_SIZE,
				file_offset: files.length,
			});
			set((state) => ({
				files: [...state.files, ...contents.files],
				loadingMore: false,
			}));
		} catch {
			set({ loadingMore: false });
		}
	},

	hasMoreFiles: () => {
		const { files, filesTotalCount } = get();
		return files.length < filesTotalCount;
	},

	setViewMode: (mode) => {
		localStorage.setItem(STORAGE_KEYS.viewMode, mode);
		set({ viewMode: mode });
	},

	setSortBy: (sortBy) => {
		localStorage.setItem(STORAGE_KEYS.sortBy, sortBy);
		set({ sortBy });
	},

	toggleSortOrder: () => {
		const newOrder = get().sortOrder === "asc" ? "desc" : "asc";
		localStorage.setItem(STORAGE_KEYS.sortOrder, newOrder);
		set({ sortOrder: newOrder });
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
		const contents = await fetchFolder(currentFolderId, initialPageParams);
		set({
			folders: contents.folders,
			files: contents.files,
			foldersTotalCount: contents.folders_total,
			filesTotalCount: contents.files_total,
		});
		return result;
	},
}));
