import {
	type ComponentProps,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import type { FileBrowserContextValue } from "@/components/files/FileBrowserContext";
import { getImagePreviewNavigation } from "@/components/files/preview/imagePreviewNavigation";
import { TagLibraryManagerDialog } from "@/components/files/TagLibraryManagerDialog";
import { TagManagerDialog } from "@/components/files/TagManagerDialog";
import { AppLayout } from "@/components/layout/AppLayout";
import type { SearchFilter } from "@/components/layout/global-search/types";
import { handleApiError } from "@/hooks/useApiError";
import { usePageTitle } from "@/hooks/usePageTitle";
import { useSelectionShortcuts } from "@/hooks/useSelectionShortcuts";
import { workspaceFolderPath, workspaceRootPath } from "@/lib/workspace";
import { FileBrowserDialogs } from "@/pages/file-browser/FileBrowserDialogs";
import { FileBrowserToolbar } from "@/pages/file-browser/FileBrowserToolbar";
import { FileBrowserWorkspace } from "@/pages/file-browser/FileBrowserWorkspace";
import type {
	FileBrowserInfoTarget,
	FileBrowserPreviewState,
	FileBrowserShareTarget,
	FileBrowserVersionTarget,
} from "@/pages/file-browser/types";
import { useFileBrowserBatchActions } from "@/pages/file-browser/useFileBrowserBatchActions";
import { useMediaQuery } from "@/pages/file-browser/useMediaQuery";
import { batchService } from "@/services/batchService";
import { fileService } from "@/services/fileService";
import { searchService } from "@/services/searchService";
import { useFileStore } from "@/stores/fileStore";
import { usePreviewAppStore } from "@/stores/previewAppStore";
import { useThumbnailSupportStore } from "@/stores/thumbnailSupportStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import type {
	FileCategory,
	FileInfo,
	FileListItem,
	FolderInfo,
	FolderListItem,
	SearchParams,
} from "@/types/api";

const SEARCH_PAGE_LIMIT = 100;
const SEARCH_TYPES = new Set<SearchFilter>(["all", "file", "folder"]);
const FILE_CATEGORIES = new Set<FileCategory>([
	"image",
	"video",
	"audio",
	"document",
	"spreadsheet",
	"presentation",
	"archive",
	"code",
	"other",
]);

type SearchTagMatch = "any" | "all";

interface ParsedSearchQuery {
	category: FileCategory | null;
	q: string;
	tagIds: string | null;
	tagMatch: SearchTagMatch;
	type: SearchFilter;
}

function parseSearchQuery(params: URLSearchParams): ParsedSearchQuery {
	const rawType = params.get("type");
	const type = SEARCH_TYPES.has(rawType as SearchFilter)
		? (rawType as SearchFilter)
		: "all";
	const rawCategory = params.get("category");
	const category = FILE_CATEGORIES.has(rawCategory as FileCategory)
		? (rawCategory as FileCategory)
		: null;
	const tagMatch = params.get("tag_match") === "all" ? "all" : "any";
	const q = params.get("q")?.trim() ?? "";
	const tagIds = params.get("tag_ids")?.trim() || null;

	return {
		category,
		q,
		tagIds,
		tagMatch,
		type: category ? "file" : type,
	};
}

function hasSearchCriteria(query: ParsedSearchQuery) {
	return Boolean(query.q || query.category || query.tagIds);
}

function buildSearchParams(
	query: ParsedSearchQuery,
	sortBy: SearchParams["sort_by"],
	sortOrder: SearchParams["sort_order"],
	offset: number,
): SearchParams {
	return {
		...(query.q ? { q: query.q } : {}),
		type: query.type,
		...(query.category ? { category: query.category } : {}),
		...(query.tagIds
			? {
					tag_ids: query.tagIds,
					tag_match: query.tagMatch,
				}
			: {}),
		sort_by: sortBy,
		sort_order: sortOrder,
		limit: SEARCH_PAGE_LIMIT,
		offset,
	};
}

export default function SearchBrowserPage() {
	const { t } = useTranslation(["core", "files", "search", "tasks"]);
	const [searchParams] = useSearchParams();
	const navigate = useNavigate();
	const workspace = useWorkspaceStore((s) => s.workspace);
	const parsedQuery = useMemo(
		() => parseSearchQuery(searchParams),
		[searchParams],
	);
	const criteriaReady = hasSearchCriteria(parsedQuery);
	const pageTitle = criteriaReady
		? `${t("core:search")}: ${
				parsedQuery.q || t(`search:category_${parsedQuery.category ?? "other"}`)
			}`
		: t("search:dialog_title");
	const searchErrorText = t("search:search_error");
	const isCompactBreadcrumb = useMediaQuery("(max-width: 639px)");
	const browserOpenMode = useFileStore((s) => s.browserOpenMode);
	const viewMode = useFileStore((s) => s.viewMode);
	const setViewMode = useFileStore((s) => s.setViewMode);
	const sortBy = useFileStore((s) => s.sortBy);
	const sortOrder = useFileStore((s) => s.sortOrder);
	const setSortBy = useFileStore((s) => s.setSortBy);
	const setSortOrder = useFileStore((s) => s.setSortOrder);
	const clearSelection = useFileStore((s) => s.clearSelection);
	const selectItems = useFileStore((s) => s.selectItems);
	const previewAppsLoaded = usePreviewAppStore((s) => s.isLoaded);
	const loadPreviewApps = usePreviewAppStore((s) => s.load);
	const thumbnailSupport = useThumbnailSupportStore((s) => s.config);
	const [files, setFiles] = useState<FileListItem[]>([]);
	const [folders, setFolders] = useState<FolderListItem[]>([]);
	const [totalFiles, setTotalFiles] = useState(0);
	const [totalFolders, setTotalFolders] = useState(0);
	const [loading, setLoading] = useState(true);
	const [loadingMore, setLoadingMore] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const [previewState, setPreviewState] =
		useState<FileBrowserPreviewState | null>(null);
	const [infoPanelOpen, setInfoPanelOpen] = useState(false);
	const [infoTarget, setInfoTarget] = useState<FileBrowserInfoTarget | null>(
		null,
	);
	const [shareTarget, setShareTarget] = useState<FileBrowserShareTarget | null>(
		null,
	);
	const [versionTarget, setVersionTarget] =
		useState<FileBrowserVersionTarget | null>(null);
	const [tagManagerOpen, setTagManagerOpen] = useState(false);
	const [tagManagerTarget, setTagManagerTarget] =
		useState<ComponentProps<typeof TagManagerDialog>["target"]>(null);
	const [tagLibraryManagerOpen, setTagLibraryManagerOpen] = useState(false);
	const [scrollViewport, setScrollViewport] = useState<HTMLDivElement | null>(
		null,
	);
	const sentinelRef = useRef<HTMLDivElement | null>(null);
	const requestIdRef = useRef(0);

	usePageTitle(pageTitle);
	const selectDisplayedItems = useCallback(() => {
		selectItems(
			files.map((file) => file.id),
			folders.map((folder) => folder.id),
		);
	}, [files, folders, selectItems]);
	useSelectionShortcuts({
		selectAll: selectDisplayedItems,
		clearSelection,
		enabled: true,
	});

	useEffect(() => {
		clearSelection();
	}, [clearSelection]);

	useEffect(() => {
		if (previewAppsLoaded) return;
		void loadPreviewApps();
	}, [loadPreviewApps, previewAppsLoaded]);

	const loadSearch = useCallback(
		async (offset: number, mode: "replace" | "append") => {
			const requestId = requestIdRef.current + 1;
			requestIdRef.current = requestId;
			if (!criteriaReady) {
				setFiles([]);
				setFolders([]);
				setTotalFiles(0);
				setTotalFolders(0);
				setLoading(false);
				setLoadingMore(false);
				setError(null);
				return;
			}
			if (mode === "replace") {
				setLoading(true);
				setError(null);
			} else {
				setLoadingMore(true);
			}

			try {
				const results = await searchService.search(
					buildSearchParams(parsedQuery, sortBy, sortOrder, offset),
				);
				if (requestIdRef.current !== requestId) {
					return;
				}
				setFiles((current) =>
					mode === "append" ? [...current, ...results.files] : results.files,
				);
				setFolders((current) =>
					mode === "append"
						? [...current, ...results.folders]
						: results.folders,
				);
				setTotalFiles(results.total_files);
				setTotalFolders(results.total_folders);
				setError(null);
			} catch (loadError) {
				if (requestIdRef.current !== requestId) {
					return;
				}
				setError(
					loadError instanceof Error ? loadError.message : searchErrorText,
				);
			} finally {
				if (requestIdRef.current === requestId) {
					setLoading(false);
					setLoadingMore(false);
				}
			}
		},
		[criteriaReady, parsedQuery, searchErrorText, sortBy, sortOrder],
	);

	useEffect(() => {
		setInfoPanelOpen(false);
		setInfoTarget(null);
		setFiles([]);
		setFolders([]);
		setTotalFiles(0);
		setTotalFolders(0);
		void loadSearch(0, "replace");
	}, [loadSearch]);

	useEffect(() => {
		if (!infoPanelOpen || !infoTarget) return;
		if (infoTarget.file) {
			const nextFile = files.find((entry) => entry.id === infoTarget.file?.id);
			if (nextFile) {
				if (nextFile !== infoTarget.file) {
					setInfoTarget({ file: nextFile });
				}
				return;
			}
		}
		if (infoTarget.folder) {
			const nextFolder = folders.find(
				(entry) => entry.id === infoTarget.folder?.id,
			);
			if (nextFolder) {
				if (nextFolder !== infoTarget.folder) {
					setInfoTarget({ folder: nextFolder });
				}
				return;
			}
		}
		setInfoPanelOpen(false);
		setInfoTarget(null);
	}, [files, folders, infoPanelOpen, infoTarget]);

	const hasMoreFiles =
		files.length < totalFiles || folders.length < totalFolders;
	useEffect(() => {
		if (!hasMoreFiles || loading || loadingMore) return;
		const sentinel = sentinelRef.current;
		if (!sentinel) return;

		const observer = new IntersectionObserver(
			(entries) => {
				if (entries[0]?.isIntersecting) {
					void loadSearch(Math.max(files.length, folders.length), "append");
				}
			},
			{ root: scrollViewport, rootMargin: "200px" },
		);
		observer.observe(sentinel);
		return () => observer.disconnect();
	}, [
		files.length,
		folders.length,
		hasMoreFiles,
		loadSearch,
		loading,
		loadingMore,
		scrollViewport,
	]);

	const handleDownload = useCallback((fileId: number, _fileName: string) => {
		const anchor = document.createElement("a");
		anchor.href = fileService.downloadUrl(fileId);
		anchor.download = "";
		anchor.click();
	}, []);

	const handleArchiveDownload = useCallback(
		(fileIds: number[], folderIds: number[]) =>
			batchService.streamArchiveDownload(fileIds, folderIds),
		[],
	);

	const { dialogs: batchActionDialogs, selectionToolbar } =
		useFileBrowserBatchActions({
			allowCopyMove: false,
			displayFiles: files,
			displayFolders: folders,
			onChanged: () => loadSearch(0, "replace"),
			onArchiveDownload: handleArchiveDownload,
			onDownload: handleDownload,
		});

	const openPreview = useCallback(
		(file: FileListItem, openMode: "auto" | "direct" | "picker") => {
			setPreviewState({ file, openMode });
		},
		[],
	);

	const navigatePreviewFile = useCallback((file: FileInfo | FileListItem) => {
		setPreviewState((current) =>
			current ? { ...current, file } : { file, openMode: "auto" },
		);
	}, []);

	const handleShare = useCallback((target: FileBrowserShareTarget) => {
		setShareTarget(target);
	}, []);

	const handleInfo = useCallback(
		(type: "file" | "folder", id: number) => {
			if (type === "file") {
				const file = files.find((entry) => entry.id === id);
				if (!file) return;
				setInfoTarget({ file });
			} else {
				const folder = folders.find((entry) => entry.id === id);
				if (!folder) return;
				setInfoTarget({ folder });
			}
			setInfoPanelOpen(true);
		},
		[files, folders],
	);

	const handleManageTags = useCallback(
		(type: "file" | "folder", id: number) => {
			const target =
				type === "file"
					? files.find((entry) => entry.id === id)
					: folders.find((entry) => entry.id === id);
			if (!target) return;

			setTagManagerTarget({
				mode: "entity",
				entityId: target.id,
				entityType: type,
				initialTags: target.tags ?? [],
				name: target.name,
				onChanged: () => loadSearch(0, "replace"),
			});
			setTagManagerOpen(true);
		},
		[files, folders, loadSearch],
	);

	const handleToggleLock = useCallback(
		async (type: "file" | "folder", id: number, locked: boolean) => {
			try {
				if (type === "file") {
					await fileService.setFileLock(id, !locked);
				} else {
					await fileService.setFolderLock(id, !locked);
				}
				toast.success(
					!locked ? t("files:lock_success") : t("files:unlock_success"),
				);
				void loadSearch(0, "replace");
				return true;
			} catch (lockError) {
				handleApiError(lockError);
				return false;
			}
		},
		[loadSearch, t],
	);

	const handleDelete = useCallback(
		async (type: "file" | "folder", id: number) => {
			try {
				if (type === "file") {
					await fileService.deleteFile(id);
				} else {
					await fileService.deleteFolder(id);
				}
				toast.success(t("files:delete_success"));
				void loadSearch(0, "replace");
			} catch (deleteError) {
				handleApiError(deleteError);
			}
		},
		[loadSearch, t],
	);

	const handleVersions = useCallback(
		(fileId: number) => {
			const file = files.find((entry) => entry.id === fileId);
			if (!file) return;
			setVersionTarget({
				fileId,
				fileName: file.name,
				mimeType: file.mime_type,
			});
		},
		[files],
	);

	const handleGoToLocation = useCallback(
		async (file: FileListItem) => {
			try {
				const info = await fileService.getFile(file.id);
				navigate(workspaceFolderPath(workspace, info.folder_id ?? null), {
					viewTransition: false,
				});
			} catch (locationError) {
				handleApiError(locationError);
			}
		},
		[navigate, workspace],
	);

	const handleFolderOpen = useCallback(
		(id: number, name: string) => {
			navigate(workspaceFolderPath(workspace, id, name), {
				viewTransition: false,
			});
		},
		[navigate, workspace],
	);

	const fileBrowserContextValue = useMemo<FileBrowserContextValue>(
		() => ({
			folders,
			files,
			browserOpenMode,
			breadcrumbPathIds: [],
			batchSelectionActions: selectionToolbar,
			onFolderOpen: handleFolderOpen,
			onFileClick: (file) => openPreview(file, "auto"),
			onFileOpen: (file) => openPreview(file, "direct"),
			onFileChooseOpenMethod: (file) => openPreview(file, "picker"),
			onShare: handleShare,
			onDownload: handleDownload,
			onArchiveDownload: (folderId) => handleArchiveDownload([], [folderId]),
			onArchiveCompress: undefined,
			onArchiveExtract: undefined,
			onManageTags: handleManageTags,
			onGoToLocation: handleGoToLocation,
			onInfo: handleInfo,
			onToggleLock: handleToggleLock,
			onDelete: handleDelete,
			onVersions: handleVersions,
			fadingFileIds: new Set<number>(),
			fadingFolderIds: new Set<number>(),
		}),
		[
			browserOpenMode,
			files,
			folders,
			handleArchiveDownload,
			handleDelete,
			handleDownload,
			handleFolderOpen,
			handleGoToLocation,
			handleInfo,
			handleManageTags,
			handleShare,
			handleToggleLock,
			handleVersions,
			openPreview,
			selectionToolbar,
		],
	);

	const previewImageNavigation = useMemo(
		() =>
			previewState
				? getImagePreviewNavigation(files, previewState.file, thumbnailSupport)
				: {},
		[files, previewState, thumbnailSupport],
	);
	const breadcrumb = useMemo(
		() => [
			{
				id: null,
				name: criteriaReady
					? `${t("core:search")}: ${
							parsedQuery.q ||
							t(`search:category_${parsedQuery.category ?? "other"}`)
						}`
					: t("search:dialog_title"),
			},
		],
		[criteriaReady, parsedQuery.category, parsedQuery.q, t],
	);

	return (
		<AppLayout>
			<FileBrowserToolbar
				breadcrumb={breadcrumb}
				currentFolderActions="refresh-only"
				dragOverBreadcrumbIndex={null}
				isCompactBreadcrumb={isCompactBreadcrumb}
				isRootFolder
				isSearching={criteriaReady}
				searchQuery={
					parsedQuery.q ||
					(parsedQuery.category
						? t(`search:category_${parsedQuery.category}`)
						: null)
				}
				selectionToolbar={selectionToolbar}
				sortBy={sortBy}
				sortOrder={sortOrder}
				uploadReady={false}
				viewMode={viewMode}
				onBreadcrumbDragLeave={() => undefined}
				onBreadcrumbDragOver={() => undefined}
				onBreadcrumbDrop={async () => undefined}
				onCreateFile={() => undefined}
				onCreateFolder={() => undefined}
				onManageTagLibrary={() => setTagLibraryManagerOpen(true)}
				onNavigateToFolder={() => navigate(workspaceRootPath(workspace))}
				onOfflineDownload={() => undefined}
				onRefresh={() => loadSearch(0, "replace")}
				onSetSortBy={setSortBy}
				onSetSortOrder={setSortOrder}
				onSetViewMode={setViewMode}
				onTriggerFileUpload={() => undefined}
				onTriggerFolderUpload={() => undefined}
			/>
			<FileBrowserWorkspace
				breadcrumb={breadcrumb}
				contentDragOver={false}
				currentFolderActions="refresh-only"
				error={error}
				fileBrowserContextValue={fileBrowserContextValue}
				hasMoreFiles={hasMoreFiles}
				infoPanelOpen={infoPanelOpen}
				infoTarget={infoTarget}
				isEmpty={!loading && files.length === 0 && folders.length === 0}
				loading={loading}
				loadingMore={loadingMore}
				scrollViewport={scrollViewport}
				sentinelRef={sentinelRef}
				uploadReady={false}
				viewMode={viewMode}
				onContentDragLeave={() => undefined}
				onContentDragOver={(event) => event.preventDefault()}
				onContentDrop={async () => undefined}
				onCreateFile={() => undefined}
				onCreateFolder={() => undefined}
				onDownload={handleDownload}
				onInfoPanelOpenChange={setInfoPanelOpen}
				onOpenInfoFolder={(folder: FolderInfo | FolderListItem) =>
					handleFolderOpen(folder.id, folder.name)
				}
				onOfflineDownload={() => undefined}
				onPreview={(file) => setPreviewState({ file, openMode: "auto" })}
				onRefresh={() => loadSearch(0, "replace")}
				onRename={() => undefined}
				onScrollViewportRef={setScrollViewport}
				onShare={handleShare}
				onToggleLock={handleToggleLock}
				onTriggerFileUpload={() => undefined}
				onTriggerFolderUpload={() => undefined}
				onVersions={handleVersions}
			/>
			<TagManagerDialog
				open={tagManagerOpen}
				onOpenChange={setTagManagerOpen}
				target={tagManagerTarget}
			/>
			<TagLibraryManagerDialog
				open={tagLibraryManagerOpen}
				onOpenChange={setTagLibraryManagerOpen}
			/>
			{batchActionDialogs}
			<FileBrowserDialogs
				archiveTaskTarget={null}
				breadcrumb={[]}
				copyTarget={null}
				createFileOpen={false}
				createFolderOpen={false}
				currentFolderId={null}
				currentFolderName={null}
				moveTarget={null}
				offlineDownloadOpen={false}
				previewImageNavigation={previewImageNavigation}
				previewState={previewState}
				renameTarget={null}
				shareTarget={shareTarget}
				versionTarget={versionTarget}
				onArchiveTaskClose={() => undefined}
				onArchiveTaskSubmit={async () => undefined}
				onCopyClose={() => undefined}
				onCopyConfirm={async () => undefined}
				onCreateFileOpenChange={() => undefined}
				onCreateFolderOpenChange={() => undefined}
				onMoveClose={() => undefined}
				onMoveConfirm={async () => undefined}
				onOfflineDownloadOpenChange={() => undefined}
				onPreviewClose={() => setPreviewState(null)}
				onPreviewFileUpdated={() => loadSearch(0, "replace")}
				onPreviewNavigate={navigatePreviewFile}
				onRenameClose={() => undefined}
				onShareClose={() => setShareTarget(null)}
				onVersionClose={() => setVersionTarget(null)}
				onVersionRestored={() => loadSearch(0, "replace")}
			/>
		</AppLayout>
	);
}
