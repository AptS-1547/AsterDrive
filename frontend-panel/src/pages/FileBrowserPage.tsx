import {
	Fragment,
	Suspense,
	useCallback,
	useEffect,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import { BatchActionBar } from "@/components/common/BatchActionBar";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonFileGrid } from "@/components/common/SkeletonFileGrid";
import { SkeletonFileTable } from "@/components/common/SkeletonFileTable";
import { SortMenu } from "@/components/common/SortMenu";
import { ToolbarBar } from "@/components/common/ToolbarBar";
import { ViewToggle } from "@/components/common/ViewToggle";
import { FileGrid } from "@/components/files/FileGrid";
import { FilePreview } from "@/components/files/FilePreview";
import { FileTable } from "@/components/files/FileTable";
import {
	UploadArea,
	type UploadAreaHandle,
} from "@/components/files/UploadArea";
import { AppLayout } from "@/components/layout/AppLayout";
import {
	Breadcrumb,
	BreadcrumbEllipsis,
	BreadcrumbItem,
	BreadcrumbLink,
	BreadcrumbList,
	BreadcrumbPage,
	BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Icon } from "@/components/ui/icon";
import { ScrollArea } from "@/components/ui/scroll-area";
import { handleApiError } from "@/hooks/useApiError";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { usePageTitle } from "@/hooks/usePageTitle";
import {
	DRAG_SOURCE_MIME,
	FILE_BROWSER_FEEDBACK_DURATION_MS,
} from "@/lib/constants";
import {
	getInvalidInternalDropReason,
	hasInternalDragData,
	type InternalDragData,
	readInternalDragData,
} from "@/lib/dragDrop";
import { formatBatchToast } from "@/lib/formatBatchToast";
import { runWhenIdle } from "@/lib/idleTask";
import { lazyWithPreload } from "@/lib/lazyWithPreload";
import { cn } from "@/lib/utils";
import { workspaceFolderPath } from "@/lib/workspace";
import { batchService } from "@/services/batchService";
import { fileService } from "@/services/fileService";
import { useAuthStore } from "@/stores/authStore";
import { useFileStore } from "@/stores/fileStore";
import { usePreviewAppStore } from "@/stores/previewAppStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import type {
	FileInfo,
	FileListItem,
	FolderInfo,
	FolderListItem,
} from "@/types/api";

const BatchTargetFolderDialog = lazyWithPreload(async () => {
	const module = await import("@/components/files/BatchTargetFolderDialog");
	return { default: module.BatchTargetFolderDialog };
});
const CreateFileDialog = lazyWithPreload(async () => {
	const module = await import("@/components/files/CreateFileDialog");
	return { default: module.CreateFileDialog };
});
const CreateFolderDialog = lazyWithPreload(async () => {
	const module = await import("@/components/files/CreateFolderDialog");
	return { default: module.CreateFolderDialog };
});
const FileInfoDialog = lazyWithPreload(async () => {
	const module = await import("@/components/files/FileInfoDialog");
	return { default: module.FileInfoDialog };
});
const RenameDialog = lazyWithPreload(async () => {
	const module = await import("@/components/files/RenameDialog");
	return { default: module.RenameDialog };
});
const ShareDialog = lazyWithPreload(async () => {
	const module = await import("@/components/files/ShareDialog");
	return { default: module.ShareDialog };
});
const VersionHistoryDialog = lazyWithPreload(async () => {
	const module = await import("@/components/files/VersionHistoryDialog");
	return { default: module.VersionHistoryDialog };
});

const FILE_BROWSER_LAZY_PRELOADERS = [
	BatchTargetFolderDialog,
	CreateFileDialog,
	CreateFolderDialog,
	FileInfoDialog,
	RenameDialog,
	ShareDialog,
	VersionHistoryDialog,
] as const;

function useMediaQuery(query: string) {
	const getMatches = () =>
		typeof window !== "undefined" &&
		typeof window.matchMedia === "function" &&
		window.matchMedia(query).matches;

	const [matches, setMatches] = useState(getMatches);

	useEffect(() => {
		if (
			typeof window === "undefined" ||
			typeof window.matchMedia !== "function"
		) {
			return;
		}

		const mediaQuery = window.matchMedia(query);
		setMatches(mediaQuery.matches);

		const handleChange = (event: MediaQueryListEvent) => {
			setMatches(event.matches);
		};

		if (typeof mediaQuery.addEventListener === "function") {
			mediaQuery.addEventListener("change", handleChange);
			return () => mediaQuery.removeEventListener("change", handleChange);
		}

		mediaQuery.addListener(handleChange);
		return () => mediaQuery.removeListener(handleChange);
	}, [query]);

	return matches;
}

type VisibleBreadcrumbEntry =
	| {
			type: "item";
			item: {
				id: number | null;
				name: string;
			};
			sourceIndex: number;
	  }
	| {
			type: "ellipsis";
			key: string;
			items: Array<{
				id: number | null;
				name: string;
			}>;
	  };

export default function FileBrowserPage() {
	const { t } = useTranslation(["files", "tasks"]);
	const navigate = useNavigate();
	const workspace = useWorkspaceStore((s) => s.workspace);
	const params = useParams<{ folderId?: string }>();
	const [searchParams] = useSearchParams();
	const folderId = params.folderId ? Number(params.folderId) : null;
	const folderName = searchParams.get("name") ?? undefined;

	const navigateTo = useFileStore((s) => s.navigateTo);
	const refresh = useFileStore((s) => s.refresh);
	const moveToFolder = useFileStore((s) => s.moveToFolder);
	const search = useFileStore((s) => s.search);
	const previewAppsLoaded = usePreviewAppStore((s) => s.isLoaded);
	const loadPreviewApps = usePreviewAppStore((s) => s.load);
	const breadcrumb = useFileStore((s) => s.breadcrumb);
	const folders = useFileStore((s) => s.folders);
	const files = useFileStore((s) => s.files);
	const loading = useFileStore((s) => s.loading);
	const viewMode = useFileStore((s) => s.viewMode);
	const browserOpenMode = useFileStore((s) => s.browserOpenMode);
	const setViewMode = useFileStore((s) => s.setViewMode);
	const searchQuery = useFileStore((s) => s.searchQuery);
	const searchFolders = useFileStore((s) => s.searchFolders);
	const searchFiles = useFileStore((s) => s.searchFiles);
	const error = useFileStore((s) => s.error);
	const clearSelection = useFileStore((s) => s.clearSelection);
	const loadMoreFiles = useFileStore((s) => s.loadMoreFiles);
	const loadingMore = useFileStore((s) => s.loadingMore);
	const sortBy = useFileStore((s) => s.sortBy);
	const sortOrder = useFileStore((s) => s.sortOrder);
	const setSortBy = useFileStore((s) => s.setSortBy);
	const setSortOrder = useFileStore((s) => s.setSortOrder);
	const hasMoreFiles = useFileStore((s) => s.hasMoreFiles());

	const isSearching = searchQuery !== null;
	const displayFolders = isSearching ? searchFolders : folders;
	const displayFiles = isSearching ? searchFiles : files;
	const currentBreadcrumbItem = breadcrumb[breadcrumb.length - 1];
	const currentFolderName = currentBreadcrumbItem?.name;
	const isRootFolder =
		currentBreadcrumbItem != null
			? currentBreadcrumbItem.id == null
			: folderId == null;
	const isCompactBreadcrumb = useMediaQuery("(max-width: 639px)");
	const pageTitle = isSearching
		? `${t("core:search")}: ${searchQuery}`
		: folderId == null
			? t("core:all_files")
			: (currentFolderName ?? t("core:all_files"));

	usePageTitle(pageTitle);

	useKeyboardShortcuts();

	const uploadAreaRef = useRef<UploadAreaHandle | null>(null);
	const sentinelRef = useRef<HTMLDivElement | null>(null);
	const [scrollViewport, setScrollViewport] = useState<HTMLDivElement | null>(
		null,
	);

	useEffect(() => {
		return runWhenIdle(() => {
			for (const preloader of FILE_BROWSER_LAZY_PRELOADERS) {
				void preloader.preload();
			}
		});
	}, []);

	// Infinite scroll: load more files when sentinel is visible
	useEffect(() => {
		if (isSearching || !hasMoreFiles || loadingMore) return;
		const el = sentinelRef.current;
		if (!el) return;
		const observer = new IntersectionObserver(
			(entries) => {
				if (entries[0].isIntersecting) {
					void loadMoreFiles();
				}
			},
			{ root: scrollViewport, rootMargin: "200px" },
		);
		observer.observe(el);
		return () => observer.disconnect();
	}, [hasMoreFiles, isSearching, loadingMore, loadMoreFiles, scrollViewport]);
	const [createFolderOpen, setCreateFolderOpen] = useState(false);
	const [createFileOpen, setCreateFileOpen] = useState(false);
	const [fadingFileIds, setFadingFileIds] = useState<Set<number>>(new Set());
	const [fadingFolderIds, setFadingFolderIds] = useState<Set<number>>(
		new Set(),
	);
	const [previewState, setPreviewState] = useState<{
		file: FileInfo | FileListItem;
		openMode: "auto" | "direct" | "picker";
	} | null>(null);
	const [shareTarget, setShareTarget] = useState<{
		fileId?: number;
		folderId?: number;
		name: string;
		initialMode?: "page" | "direct";
	} | null>(null);
	const [copyTarget, setCopyTarget] = useState<{
		type: "file" | "folder";
		id: number;
	} | null>(null);
	const [moveTarget, setMoveTarget] = useState<{
		fileIds: number[];
		folderIds: number[];
	} | null>(null);
	const [versionTarget, setVersionTarget] = useState<{
		fileId: number;
		fileName: string;
		mimeType: string;
	} | null>(null);
	const [dragOverBreadcrumbIndex, setDragOverBreadcrumbIndex] = useState<
		number | null
	>(null);
	const [contentDragOver, setContentDragOver] = useState(false);
	const [renameTarget, setRenameTarget] = useState<{
		type: "file" | "folder";
		id: number;
		name: string;
	} | null>(null);
	const [infoPanelOpen, setInfoPanelOpen] = useState(false);
	const [infoTarget, setInfoTarget] = useState<{
		file?: FileInfo | FileListItem;
		folder?: FolderInfo | FolderListItem;
	} | null>(null);

	useEffect(() => {
		setInfoPanelOpen(false);
		setInfoTarget(null);
		navigateTo(folderId, folderName).catch(handleApiError);
	}, [folderId, folderName, navigateTo]);

	useEffect(() => {
		if (previewAppsLoaded) return;
		void loadPreviewApps();
	}, [loadPreviewApps, previewAppsLoaded]);

	useEffect(() => {
		if (!infoPanelOpen || infoTarget == null) {
			return;
		}

		if (infoTarget.file) {
			const nextFile = displayFiles.find(
				(entry) => entry.id === infoTarget.file?.id,
			);
			if (nextFile && nextFile !== infoTarget.file) {
				setInfoTarget({ file: nextFile });
			}
			return;
		}

		if (infoTarget.folder) {
			const nextFolder = displayFolders.find(
				(entry) => entry.id === infoTarget.folder?.id,
			);
			if (nextFolder && nextFolder !== infoTarget.folder) {
				setInfoTarget({ folder: nextFolder });
			}
		}
	}, [displayFiles, displayFolders, infoPanelOpen, infoTarget]);

	useEffect(() => {
		function onRenameRequest(e: Event) {
			const { type, id, name } = (e as CustomEvent).detail as {
				type: "file" | "folder";
				id: number;
				name: string;
			};
			void RenameDialog.preload();
			setRenameTarget({ type, id, name });
		}
		document.addEventListener("rename-request", onRenameRequest);
		return () =>
			document.removeEventListener("rename-request", onRenameRequest);
	}, []);

	const handleDownload = useCallback((fileId: number, _fileName: string) => {
		const a = document.createElement("a");
		a.href = fileService.downloadUrl(fileId);
		a.download = "";
		a.click();
	}, []);

	const handleCopy = useCallback((type: "file" | "folder", id: number) => {
		void BatchTargetFolderDialog.preload();
		setCopyTarget({ type, id });
	}, []);

	const handleCopyConfirm = useCallback(
		async (targetFolderId: number | null) => {
			if (!copyTarget) return;
			try {
				if (copyTarget.type === "file") {
					await fileService.copyFile(copyTarget.id, targetFolderId);
				} else {
					await fileService.copyFolder(copyTarget.id, targetFolderId);
				}
				toast.success(t("copy_success"));
				setCopyTarget(null);
				await refresh();
			} catch (err) {
				handleApiError(err);
			}
		},
		[copyTarget, refresh, t],
	);

	const handleMove = useCallback((type: "file" | "folder", id: number) => {
		void BatchTargetFolderDialog.preload();
		setMoveTarget(
			type === "file"
				? { fileIds: [id], folderIds: [] }
				: { fileIds: [], folderIds: [id] },
		);
	}, []);

	const handleToggleLock = useCallback(
		async (type: "file" | "folder", id: number, locked: boolean) => {
			try {
				if (type === "file") await fileService.setFileLock(id, !locked);
				else await fileService.setFolderLock(id, !locked);
				toast.success(!locked ? t("lock_success") : t("unlock_success"));
				refresh();
				return true;
			} catch (err) {
				handleApiError(err);
				return false;
			}
		},
		[refresh, t],
	);

	const handleVersions = useCallback(
		(fileId: number) => {
			const targetFile = displayFiles.find((entry) => entry.id === fileId);
			if (!targetFile) return;
			void VersionHistoryDialog.preload();
			setVersionTarget({
				fileId,
				fileName: targetFile.name,
				mimeType: targetFile.mime_type,
			});
		},
		[displayFiles],
	);

	const openPreview = useCallback(
		(file: FileInfo | FileListItem, openMode: "auto" | "direct" | "picker") => {
			setPreviewState({ file, openMode });
		},
		[],
	);

	const openShareDialog = useCallback(
		(target: {
			fileId?: number;
			folderId?: number;
			name: string;
			initialMode?: "page" | "direct";
		}) => {
			void ShareDialog.preload();
			setShareTarget(target);
		},
		[],
	);

	const openRenameDialog = useCallback(
		(type: "file" | "folder", id: number, name: string) => {
			void RenameDialog.preload();
			setRenameTarget({ type, id, name });
		},
		[],
	);

	const handleDelete = useCallback(
		async (type: "file" | "folder", id: number) => {
			try {
				if (type === "file") await useFileStore.getState().deleteFile(id);
				else await useFileStore.getState().deleteFolder(id);
				toast.success(t("delete_success"));
			} catch (err) {
				handleApiError(err);
			}
		},
		[t],
	);

	const startArchiveDownload = useCallback(
		async (fileIds: number[], folderIds: number[]) => {
			if (fileIds.length === 0 && folderIds.length === 0) {
				return;
			}

			await batchService.streamArchiveDownload(fileIds, folderIds);
		},
		[],
	);

	const handleMoveToFolder = useCallback(
		async (
			fileIds: number[],
			folderIds: number[],
			targetFolderId: number | null,
		) => {
			try {
				// Fade out moved items before refreshing
				setFadingFileIds(new Set(fileIds));
				setFadingFolderIds(new Set(folderIds));
				const result = await moveToFolder(fileIds, folderIds, targetFolderId);
				document.dispatchEvent(
					new CustomEvent("folder-tree-move", {
						detail: { folderIds, targetFolderId },
					}),
				);
				// Keep local feedback brief, closer to desktop file managers.
				await new Promise((r) =>
					setTimeout(r, FILE_BROWSER_FEEDBACK_DURATION_MS),
				);
				setFadingFileIds(new Set());
				setFadingFolderIds(new Set());
				const batchToast = formatBatchToast(t, "move", result);
				if (batchToast.variant === "error") {
					toast.error(batchToast.title, {
						description: batchToast.description,
					});
				} else {
					toast.success(batchToast.title, {
						description: batchToast.description,
					});
				}
			} catch (err) {
				setFadingFileIds(new Set());
				setFadingFolderIds(new Set());
				handleApiError(err);
			}
		},
		[moveToFolder, t],
	);

	const handleMoveConfirm = useCallback(
		async (targetFolderId: number | null) => {
			if (!moveTarget) return;
			await handleMoveToFolder(
				moveTarget.fileIds,
				moveTarget.folderIds,
				targetFolderId,
			);
			setMoveTarget(null);
		},
		[handleMoveToFolder, moveTarget],
	);

	const handleBreadcrumbDragOver = useCallback(
		(e: React.DragEvent, index: number) => {
			if (!hasInternalDragData(e.dataTransfer)) return;
			e.preventDefault();
			e.dataTransfer.dropEffect = "move";
			setDragOverBreadcrumbIndex(index);
		},
		[],
	);

	const handleBreadcrumbDragLeave = useCallback((e: React.DragEvent) => {
		const nextTarget = e.relatedTarget;
		if (nextTarget instanceof Node && e.currentTarget.contains(nextTarget)) {
			return;
		}
		setDragOverBreadcrumbIndex(null);
	}, []);

	const handleBreadcrumbDrop = useCallback(
		async (
			e: React.DragEvent,
			index: number,
			targetFolderId: number | null,
		) => {
			setDragOverBreadcrumbIndex(null);
			e.preventDefault();
			const data = readInternalDragData(e.dataTransfer);
			if (!data) return;

			const targetPathIds = breadcrumb
				.slice(0, index + 1)
				.map((item) => item.id)
				.filter((id): id is number => id !== null);
			if (
				getInvalidInternalDropReason(data, targetFolderId, targetPathIds) !==
				null
			) {
				return;
			}

			await handleMoveToFolder(data.fileIds, data.folderIds, targetFolderId);
		},
		[breadcrumb, handleMoveToFolder],
	);

	const handleContentDragOver = useCallback(
		(e: React.DragEvent<HTMLDivElement>) => {
			const isTreeDrag = e.dataTransfer.types.includes(DRAG_SOURCE_MIME);
			if (!hasInternalDragData(e.dataTransfer) || isSearching || !isTreeDrag) {
				setContentDragOver(false);
				return;
			}
			e.preventDefault();
			e.dataTransfer.dropEffect = "move";
			setContentDragOver(true);
		},
		[isSearching],
	);

	const handleContentDragLeave = useCallback(
		(e: React.DragEvent<HTMLDivElement>) => {
			const nextTarget = e.relatedTarget;
			if (nextTarget instanceof Node && e.currentTarget.contains(nextTarget)) {
				return;
			}
			setContentDragOver(false);
		},
		[],
	);

	const handleContentDrop = useCallback(
		async (e: React.DragEvent<HTMLDivElement>) => {
			setContentDragOver(false);
			if (isSearching || !e.dataTransfer.types.includes(DRAG_SOURCE_MIME))
				return;
			e.preventDefault();
			const data = readInternalDragData(e.dataTransfer);
			if (!data) return;
			const currentPathIds = breadcrumb
				.map((item) => item.id)
				.filter((id): id is number => id !== null);
			if (
				getInvalidInternalDropReason(data, folderId, currentPathIds) !== null
			) {
				return;
			}
			await handleMoveToFolder(data.fileIds, data.folderIds, folderId);
		},
		[breadcrumb, folderId, handleMoveToFolder, isSearching],
	);

	const handleTrashDrop = useCallback(
		async ({ fileIds, folderIds }: InternalDragData) => {
			if (fileIds.length === 0 && folderIds.length === 0) return;
			try {
				setFadingFileIds(new Set(fileIds));
				setFadingFolderIds(new Set(folderIds));
				const result = await batchService.batchDelete(fileIds, folderIds);
				await new Promise((resolve) =>
					setTimeout(resolve, FILE_BROWSER_FEEDBACK_DURATION_MS),
				);
				setFadingFileIds(new Set());
				setFadingFolderIds(new Set());
				const batchToast = formatBatchToast(t, "delete", result);
				if (batchToast.variant === "error") {
					toast.error(batchToast.title, {
						description: batchToast.description,
					});
				} else {
					toast.success(batchToast.title, {
						description: batchToast.description,
					});
				}
				clearSelection();
				await Promise.all([
					searchQuery ? search(searchQuery) : refresh(),
					useAuthStore.getState().refreshUser(),
				]);
			} catch (err) {
				setFadingFileIds(new Set());
				setFadingFolderIds(new Set());
				handleApiError(err);
			}
		},
		[clearSelection, refresh, search, searchQuery, t],
	);

	const breadcrumbPathIds = breadcrumb
		.map((item) => item.id)
		.filter((id): id is number => id !== null);

	const sharedProps = {
		folders: displayFolders,
		files: displayFiles,
		browserOpenMode,
		scrollElement: scrollViewport,
		breadcrumbPathIds,
		onFolderOpen: (id: number, name: string) =>
			navigate(workspaceFolderPath(workspace, id, name)),
		onFileClick: (file: FileListItem) => openPreview(file, "auto"),
		onFileOpen: (file: FileListItem) => openPreview(file, "direct"),
		onFileChooseOpenMethod: (file: FileListItem) => openPreview(file, "picker"),
		onShare: openShareDialog,
		onDownload: handleDownload,
		onArchiveDownload: (folderId: number) => {
			void startArchiveDownload([], [folderId]).catch(handleApiError);
		},
		onCopy: handleCopy,
		onMove: handleMove,
		onToggleLock: handleToggleLock,
		onDelete: handleDelete,
		onRename: openRenameDialog,
		onVersions: handleVersions,
		onInfo: (type: "file" | "folder", id: number) => {
			void FileInfoDialog.preload();
			if (type === "file") {
				const f = displayFiles.find((f) => f.id === id);
				if (f) {
					setInfoTarget({ file: f });
					setInfoPanelOpen(true);
				}
			} else {
				const folder = displayFolders.find((f) => f.id === id);
				if (folder) {
					setInfoTarget({ folder });
					setInfoPanelOpen(true);
				}
			}
		},
		onMoveToFolder: handleMoveToFolder,
		fadingFileIds,
		fadingFolderIds,
	};

	const isEmpty =
		!loading && displayFolders.length === 0 && displayFiles.length === 0;
	const uploadReady = uploadAreaRef.current !== null;
	const handleUploadAreaReady = useCallback(
		(instance: UploadAreaHandle | null) => {
			uploadAreaRef.current = instance;
		},
		[],
	);
	const handleScrollViewportRef = useCallback((node: HTMLDivElement | null) => {
		setScrollViewport(node);
	}, []);
	const visibleBreadcrumb: VisibleBreadcrumbEntry[] =
		isCompactBreadcrumb && breadcrumb.length > 2
			? [
					{ type: "item", item: breadcrumb[0], sourceIndex: 0 },
					{
						type: "ellipsis",
						key: "ellipsis",
						items: breadcrumb.slice(1, -1),
					},
					{
						type: "item",
						item: breadcrumb[breadcrumb.length - 1],
						sourceIndex: breadcrumb.length - 1,
					},
				]
			: breadcrumb.map((item, index) => ({
					type: "item" as const,
					item,
					sourceIndex: index,
				}));
	const pageCore = (
		<>
			{/* Breadcrumb / local controls */}
			<ToolbarBar
				left={
					<>
						<span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-background/40 text-muted-foreground/70 sm:h-8 sm:w-8">
							<Icon
								name={isRootFolder ? "House" : "FolderOpen"}
								className="h-4 w-4"
							/>
						</span>
						<div className="min-w-0 flex-1">
							{isSearching ? (
								<span className="block truncate text-xs text-muted-foreground sm:text-sm">
									{t("core:search")}: &quot;{searchQuery}&quot;
								</span>
							) : (
								<Breadcrumb className="min-w-0">
									<BreadcrumbList className="min-w-0 gap-1.5 text-xs sm:gap-2 sm:text-sm">
										{visibleBreadcrumb.map((entry, i) => (
											<Fragment
												key={
													entry.type === "ellipsis"
														? entry.key
														: `${entry.item.id ?? "root"}-${entry.sourceIndex}`
												}
											>
												{i > 0 && (
													<BreadcrumbSeparator className="mx-0.5 text-muted-foreground/45" />
												)}
												{entry.type === "ellipsis" ? (
													<BreadcrumbItem className="shrink-0">
														<DropdownMenu>
															<DropdownMenuTrigger
																render={
																	<button
																		type="button"
																		className="flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground sm:h-7 sm:w-7"
																		aria-label={t("core:more")}
																	>
																		<BreadcrumbEllipsis />
																	</button>
																}
															/>
															<DropdownMenuContent
																align="start"
																className="w-auto min-w-40"
															>
																{entry.items.map((hiddenItem) => (
																	<DropdownMenuItem
																		key={hiddenItem.id ?? "root"}
																		onClick={() =>
																			navigate(
																				workspaceFolderPath(
																					workspace,
																					hiddenItem.id,
																					hiddenItem.name,
																				),
																			)
																		}
																	>
																		<Icon
																			name="FolderOpen"
																			className="h-4 w-4 text-muted-foreground"
																		/>
																		<span className="truncate">
																			{hiddenItem.name}
																		</span>
																	</DropdownMenuItem>
																))}
															</DropdownMenuContent>
														</DropdownMenu>
													</BreadcrumbItem>
												) : (
													<BreadcrumbItem
														className={
															entry.sourceIndex === breadcrumb.length - 1
																? "min-w-0 flex-1"
																: "shrink-0"
														}
													>
														{entry.sourceIndex < breadcrumb.length - 1 ? (
															<BreadcrumbLink
																className={[
																	"cursor-pointer rounded-md px-1 py-0.5 text-[13px] text-muted-foreground sm:px-1.5 sm:text-sm",
																	dragOverBreadcrumbIndex ===
																		entry.sourceIndex &&
																		"ring-2 ring-primary bg-accent/30 text-foreground",
																]
																	.filter(Boolean)
																	.join(" ")}
																onDragOver={(e) =>
																	handleBreadcrumbDragOver(e, entry.sourceIndex)
																}
																onDragLeave={handleBreadcrumbDragLeave}
																onDrop={(e) =>
																	handleBreadcrumbDrop(
																		e,
																		entry.sourceIndex,
																		entry.item.id,
																	)
																}
																onClick={() =>
																	navigate(
																		workspaceFolderPath(
																			workspace,
																			entry.item.id,
																			entry.item.name,
																		),
																	)
																}
															>
																{entry.item.name}
															</BreadcrumbLink>
														) : (
															<BreadcrumbPage className="text-sm font-semibold text-foreground sm:text-[0.95rem]">
																{entry.item.name}
															</BreadcrumbPage>
														)}
													</BreadcrumbItem>
												)}
											</Fragment>
										))}
									</BreadcrumbList>
								</Breadcrumb>
							)}
						</div>
						<button
							type="button"
							className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-background/40 text-muted-foreground transition-colors hover:bg-background/70 hover:text-foreground sm:h-8 sm:w-8"
							onClick={() => void refresh()}
							aria-label={t("core:refresh")}
							title={t("core:refresh")}
						>
							<Icon name="ArrowsClockwise" className="h-4 w-4" />
						</button>
					</>
				}
				right={
					<>
						<SortMenu
							sortBy={sortBy}
							sortOrder={sortOrder}
							onSortBy={setSortBy}
							onSortOrder={setSortOrder}
						/>
						<ViewToggle value={viewMode} onChange={setViewMode} />
					</>
				}
			/>
			<div className="min-h-0 flex flex-1">
				<section
					aria-label={t("file_drop_zone")}
					className={cn(
						"relative flex min-h-0 min-w-0 flex-1 flex-col transition-colors",
						contentDragOver && "bg-accent/10",
					)}
					onDragOver={handleContentDragOver}
					onDragLeave={handleContentDragLeave}
					onDrop={handleContentDrop}
				>
					{contentDragOver && (
						<div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center bg-background/35 backdrop-blur-[2px]">
							<div className="flex items-center gap-3 rounded-2xl bg-background/80 px-4 py-3 shadow-lg shadow-black/5 ring-1 ring-border/50 backdrop-blur-md">
								<div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10 text-primary">
									<Icon name="FolderOpen" className="h-5 w-5" />
								</div>
								<div className="space-y-0.5">
									<div className="text-sm font-semibold text-foreground">
										{t("move_to_current_folder")}
									</div>
									<div className="max-w-56 truncate text-xs text-muted-foreground">
										{breadcrumb[breadcrumb.length - 1]?.name ?? t("root")}
									</div>
								</div>
							</div>
						</div>
					)}
					<ContextMenu>
						<ContextMenuTrigger className="flex min-h-0 flex-1 flex-col">
							<ScrollArea
								ref={handleScrollViewportRef}
								className="min-h-0 flex-1"
							>
								{loading ? (
									viewMode === "grid" ? (
										<SkeletonFileGrid />
									) : (
										<SkeletonFileTable />
									)
								) : error ? (
									<EmptyState
										icon={<Icon name="Warning" className="h-12 w-12" />}
										title={t("core:error")}
										description={error}
									/>
								) : isEmpty ? (
									<EmptyState
										icon={<Icon name="FolderOpen" className="h-12 w-12" />}
										title={t("folder_empty")}
										description={t("folder_empty_desc")}
									/>
								) : viewMode === "grid" ? (
									<FileGrid {...sharedProps} />
								) : (
									<FileTable {...sharedProps} />
								)}
								{!isSearching && hasMoreFiles && (
									<div ref={sentinelRef} className="flex justify-center py-4">
										{loadingMore && (
											<div className="h-5 w-5 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-muted-foreground" />
										)}
									</div>
								)}
							</ScrollArea>
						</ContextMenuTrigger>
						<ContextMenuContent>
							<ContextMenuItem
								disabled={!uploadReady}
								onClick={() => uploadAreaRef.current?.triggerFileUpload()}
							>
								<Icon name="Upload" className="mr-2 h-4 w-4" />
								{t("upload_file")}
							</ContextMenuItem>
							<ContextMenuItem
								disabled={!uploadReady}
								onClick={() => uploadAreaRef.current?.triggerFolderUpload()}
							>
								<Icon name="FolderOpen" className="mr-2 h-4 w-4" />
								{t("upload_folder")}
							</ContextMenuItem>
							<ContextMenuSeparator />
							<ContextMenuItem onClick={() => setCreateFolderOpen(true)}>
								<Icon name="FolderPlus" className="mr-2 h-4 w-4" />
								{t("new_folder")}
							</ContextMenuItem>
							<ContextMenuItem onClick={() => setCreateFileOpen(true)}>
								<Icon name="FilePlus" className="mr-2 h-4 w-4" />
								{t("new_file")}
							</ContextMenuItem>
							<ContextMenuSeparator />
							<ContextMenuItem onClick={() => void refresh()}>
								<Icon name="ArrowsClockwise" className="mr-2 h-4 w-4" />
								{t("core:refresh")}
							</ContextMenuItem>
						</ContextMenuContent>
					</ContextMenu>
				</section>
				<Suspense fallback={null}>
					<FileInfoDialog
						open={infoPanelOpen}
						onOpenChange={(open) => {
							setInfoPanelOpen(open);
						}}
						file={infoTarget?.file}
						folder={infoTarget?.folder}
						onPreview={(targetFile) => openPreview(targetFile, "auto")}
						onOpenFolder={(targetFolder) =>
							navigate(
								workspaceFolderPath(
									workspace,
									targetFolder.id,
									targetFolder.name,
								),
							)
						}
						onShare={openShareDialog}
						onDownload={handleDownload}
						onRename={openRenameDialog}
						onVersions={handleVersions}
						onToggleLock={handleToggleLock}
					/>
				</Suspense>
			</div>
		</>
	);

	return (
		<AppLayout
			onTrashDrop={handleTrashDrop}
			onMoveToFolder={handleMoveToFolder}
		>
			<UploadArea ref={handleUploadAreaReady}>{pageCore}</UploadArea>

			<Suspense fallback={null}>
				<CreateFolderDialog
					open={createFolderOpen}
					onOpenChange={setCreateFolderOpen}
				/>
			</Suspense>

			<Suspense fallback={null}>
				<CreateFileDialog
					open={createFileOpen}
					onOpenChange={setCreateFileOpen}
				/>
			</Suspense>

			<BatchActionBar onArchiveDownload={startArchiveDownload} />

			<Suspense fallback={null}>
				<ShareDialog
					open={shareTarget !== null}
					onOpenChange={(open) => {
						if (!open) setShareTarget(null);
					}}
					fileId={shareTarget?.fileId}
					folderId={shareTarget?.folderId}
					name={shareTarget?.name ?? ""}
					initialMode={shareTarget?.initialMode}
				/>
			</Suspense>
			{previewState && (
				<FilePreview
					file={previewState.file}
					openMode={previewState.openMode}
					onClose={() => setPreviewState(null)}
					onFileUpdated={() => refresh()}
					previewLinkFactory={() =>
						fileService.createPreviewLink(previewState.file.id)
					}
					wopiSessionFactory={(appKey) =>
						fileService.createWopiSession(previewState.file.id, appKey)
					}
				/>
			)}
			<Suspense fallback={null}>
				<BatchTargetFolderDialog
					open={copyTarget !== null}
					onOpenChange={(open) => {
						if (!open) setCopyTarget(null);
					}}
					mode="copy"
					onConfirm={handleCopyConfirm}
					currentFolderId={folderId}
					initialBreadcrumb={breadcrumb}
					selectedFolderIds={
						copyTarget?.type === "folder" ? [copyTarget.id] : []
					}
				/>
			</Suspense>
			<Suspense fallback={null}>
				<BatchTargetFolderDialog
					open={moveTarget !== null}
					onOpenChange={(open) => {
						if (!open) setMoveTarget(null);
					}}
					mode="move"
					onConfirm={handleMoveConfirm}
					currentFolderId={folderId}
					initialBreadcrumb={breadcrumb}
					selectedFolderIds={moveTarget?.folderIds ?? []}
				/>
			</Suspense>
			<Suspense fallback={null}>
				<VersionHistoryDialog
					open={versionTarget !== null}
					onOpenChange={(open) => {
						if (!open) setVersionTarget(null);
					}}
					fileId={versionTarget?.fileId ?? 0}
					fileName={versionTarget?.fileName ?? ""}
					mimeType={versionTarget?.mimeType}
					onRestored={() => {
						setVersionTarget(null);
						refresh();
					}}
				/>
			</Suspense>
			<Suspense fallback={null}>
				<RenameDialog
					open={renameTarget !== null}
					onOpenChange={(open) => {
						if (!open) setRenameTarget(null);
					}}
					type={renameTarget?.type ?? "file"}
					id={renameTarget?.id ?? 0}
					currentName={renameTarget?.name ?? ""}
				/>
			</Suspense>
		</AppLayout>
	);
}
