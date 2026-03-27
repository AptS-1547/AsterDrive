import {
	Fragment,
	lazy,
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
import { FileTable } from "@/components/files/FileTable";
import type { UploadAreaHandle } from "@/components/files/UploadArea";
import { AppLayout } from "@/components/layout/AppLayout";
import {
	Breadcrumb,
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
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Icon } from "@/components/ui/icon";
import { ScrollArea } from "@/components/ui/scroll-area";
import { handleApiError } from "@/hooks/useApiError";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { DRAG_SOURCE_MIME } from "@/lib/constants";
import {
	getInvalidInternalDropReason,
	hasInternalDragData,
	type InternalDragData,
	readInternalDragData,
} from "@/lib/dragDrop";
import { formatBatchToast } from "@/lib/formatBatchToast";
import { cn } from "@/lib/utils";
import { batchService } from "@/services/batchService";
import { fileService } from "@/services/fileService";
import { useAuthStore } from "@/stores/authStore";
import { useFileStore } from "@/stores/fileStore";
import type {
	FileInfo,
	FileListItem,
	FolderInfo,
	FolderListItem,
} from "@/types/api";

const BatchTargetFolderDialog = lazy(async () => {
	const module = await import("@/components/files/BatchTargetFolderDialog");
	return { default: module.BatchTargetFolderDialog };
});
const CreateFileDialog = lazy(async () => {
	const module = await import("@/components/files/CreateFileDialog");
	return { default: module.CreateFileDialog };
});
const CreateFolderDialog = lazy(async () => {
	const module = await import("@/components/files/CreateFolderDialog");
	return { default: module.CreateFolderDialog };
});
const FileInfoDialog = lazy(async () => {
	const module = await import("@/components/files/FileInfoDialog");
	return { default: module.FileInfoDialog };
});
const FilePreview = lazy(async () => {
	const module = await import("@/components/files/FilePreview");
	return { default: module.FilePreview };
});
const RenameDialog = lazy(async () => {
	const module = await import("@/components/files/RenameDialog");
	return { default: module.RenameDialog };
});
const ShareDialog = lazy(async () => {
	const module = await import("@/components/files/ShareDialog");
	return { default: module.ShareDialog };
});
const VersionHistoryDialog = lazy(async () => {
	const module = await import("@/components/files/VersionHistoryDialog");
	return { default: module.VersionHistoryDialog };
});
const UploadArea = lazy(async () => {
	const module = await import("@/components/files/UploadArea");
	return { default: module.UploadArea };
});

export default function FileBrowserPage() {
	const { t } = useTranslation("files");
	const navigate = useNavigate();
	const params = useParams<{ folderId?: string }>();
	const [searchParams] = useSearchParams();
	const folderId = params.folderId ? Number(params.folderId) : null;
	const folderName = searchParams.get("name") ?? undefined;

	const navigateTo = useFileStore((s) => s.navigateTo);
	const refresh = useFileStore((s) => s.refresh);
	const moveToFolder = useFileStore((s) => s.moveToFolder);
	const search = useFileStore((s) => s.search);
	const breadcrumb = useFileStore((s) => s.breadcrumb);
	const folders = useFileStore((s) => s.folders);
	const files = useFileStore((s) => s.files);
	const loading = useFileStore((s) => s.loading);
	const viewMode = useFileStore((s) => s.viewMode);
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

	const hasMoreFiles = useFileStore((s) => s.hasMoreFiles);

	const isSearching = searchQuery !== null;
	const displayFolders = isSearching ? searchFolders : folders;
	const displayFiles = isSearching ? searchFiles : files;

	useKeyboardShortcuts();

	const uploadAreaRef = useRef<UploadAreaHandle | null>(null);
	const sentinelRef = useRef<HTMLDivElement | null>(null);
	const scrollAreaRef = useRef<HTMLDivElement | null>(null);

	// Infinite scroll: load more files when sentinel is visible
	useEffect(() => {
		if (isSearching || !hasMoreFiles() || loadingMore) return;
		const el = sentinelRef.current;
		if (!el) return;
		// scrollAreaRef.current is the Viewport element (via forwardRef)
		const observer = new IntersectionObserver(
			(entries) => {
				if (entries[0].isIntersecting) {
					void loadMoreFiles();
				}
			},
			{ root: scrollAreaRef.current ?? null, rootMargin: "200px" },
		);
		observer.observe(el);
		return () => observer.disconnect();
	}, [isSearching, hasMoreFiles, loadingMore, loadMoreFiles]);
	const [createFolderOpen, setCreateFolderOpen] = useState(false);
	const [createFileOpen, setCreateFileOpen] = useState(false);
	const [fadingFileIds, setFadingFileIds] = useState<Set<number>>(new Set());
	const [fadingFolderIds, setFadingFolderIds] = useState<Set<number>>(
		new Set(),
	);
	const [previewFile, setPreviewFile] = useState<
		FileInfo | FileListItem | null
	>(null);
	const [shareTarget, setShareTarget] = useState<{
		fileId?: number;
		folderId?: number;
		name: string;
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
		size: number;
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
	const [infoTarget, setInfoTarget] = useState<{
		file?: FileInfo | FileListItem;
		folder?: FolderInfo | FolderListItem;
	} | null>(null);

	useEffect(() => {
		navigateTo(folderId, folderName).catch(handleApiError);
	}, [folderId, folderName, navigateTo]);

	useEffect(() => {
		function onRenameRequest(e: Event) {
			const { type, id, name } = (e as CustomEvent).detail as {
				type: "file" | "folder";
				id: number;
				name: string;
			};
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
			} catch (err) {
				handleApiError(err);
			}
		},
		[refresh, t],
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
				// Wait for fade-out animation to finish, then clear
				await new Promise((r) => setTimeout(r, 300));
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
				await new Promise((resolve) => setTimeout(resolve, 300));
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
		breadcrumbPathIds,
		onFolderOpen: (id: number, name: string) =>
			navigate(`/folder/${id}?name=${encodeURIComponent(name)}`),
		onFileClick: (file: FileListItem) => setPreviewFile(file),
		onShare: setShareTarget,
		onDownload: handleDownload,
		onCopy: handleCopy,
		onMove: handleMove,
		onToggleLock: handleToggleLock,
		onDelete: handleDelete,
		onRename: (type: "file" | "folder", id: number, name: string) =>
			setRenameTarget({ type, id, name }),
		onVersions: (fileId: number) => {
			const targetFile = displayFiles.find((file) => file.id === fileId);
			if (!targetFile) return;
			setVersionTarget({
				fileId,
				fileName: targetFile.name,
				mimeType: targetFile.mime_type,
				size: targetFile.size,
			});
		},
		onInfo: (type: "file" | "folder", id: number) => {
			if (type === "file") {
				const f = displayFiles.find((f) => f.id === id);
				if (f) setInfoTarget({ file: f });
			} else {
				const folder = displayFolders.find((f) => f.id === id);
				if (folder) setInfoTarget({ folder });
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
	const pageCore = (
		<>
			{/* Breadcrumb / local controls */}
			<ToolbarBar
				left={
					<>
						<Icon
							name="FolderOpen"
							className="h-4 w-4 shrink-0 text-muted-foreground/70"
						/>
						<div className="min-w-0 flex-1">
							{isSearching ? (
								<span className="text-sm text-muted-foreground">
									{t("common:search")}: &quot;{searchQuery}&quot;
								</span>
							) : (
								<Breadcrumb>
									<BreadcrumbList className="gap-2">
										{breadcrumb.map((item, i) => (
											<Fragment key={item.id ?? "root"}>
												{i > 0 && (
													<BreadcrumbSeparator className="mx-0.5 text-muted-foreground/45" />
												)}
												<BreadcrumbItem>
													{i < breadcrumb.length - 1 ? (
														<BreadcrumbLink
															className={[
																"cursor-pointer rounded-md px-1.5 py-0.5 text-muted-foreground",
																dragOverBreadcrumbIndex === i &&
																	"ring-2 ring-primary bg-accent/30 text-foreground",
															]
																.filter(Boolean)
																.join(" ")}
															onDragOver={(e) => handleBreadcrumbDragOver(e, i)}
															onDragLeave={handleBreadcrumbDragLeave}
															onDrop={(e) =>
																handleBreadcrumbDrop(e, i, item.id)
															}
															onClick={() =>
																navigate(
																	item.id === null
																		? "/"
																		: `/folder/${item.id}?name=${encodeURIComponent(item.name)}`,
																)
															}
														>
															{item.name}
														</BreadcrumbLink>
													) : (
														<BreadcrumbPage className="text-base font-semibold text-foreground">
															{item.name}
														</BreadcrumbPage>
													)}
												</BreadcrumbItem>
											</Fragment>
										))}
									</BreadcrumbList>
								</Breadcrumb>
							)}
						</div>
						<button
							type="button"
							className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-background/70 hover:text-foreground"
							onClick={() => void refresh()}
							aria-label={t("refresh")}
							title={t("refresh")}
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
			<div
				className={cn(
					"relative min-h-0 flex-1 transition-colors",
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
						<ScrollArea ref={scrollAreaRef} className="min-h-0 flex-1">
							{loading ? (
								viewMode === "grid" ? (
									<SkeletonFileGrid />
								) : (
									<SkeletonFileTable />
								)
							) : error ? (
								<EmptyState
									icon={<Icon name="Warning" className="h-12 w-12" />}
									title={t("common:error")}
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
							{!isSearching && hasMoreFiles() && (
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
						<ContextMenuItem onClick={() => setCreateFolderOpen(true)}>
							<Icon name="FolderPlus" className="mr-2 h-4 w-4" />
							{t("new_folder")}
						</ContextMenuItem>
						<ContextMenuItem onClick={() => setCreateFileOpen(true)}>
							<Icon name="FilePlus" className="mr-2 h-4 w-4" />
							{t("new_file")}
						</ContextMenuItem>
					</ContextMenuContent>
				</ContextMenu>
			</div>
		</>
	);

	return (
		<AppLayout
			onTrashDrop={handleTrashDrop}
			onMoveToFolder={handleMoveToFolder}
		>
			<Suspense fallback={pageCore}>
				<UploadArea ref={handleUploadAreaReady}>{pageCore}</UploadArea>
			</Suspense>

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

			<BatchActionBar />

			{shareTarget && (
				<Suspense fallback={null}>
					<ShareDialog
						open={true}
						onOpenChange={(open) => {
							if (!open) setShareTarget(null);
						}}
						fileId={shareTarget.fileId}
						folderId={shareTarget.folderId}
						name={shareTarget.name}
					/>
				</Suspense>
			)}
			{previewFile && (
				<Suspense fallback={null}>
					<FilePreview
						file={previewFile}
						onClose={() => setPreviewFile(null)}
						onFileUpdated={() => refresh()}
					/>
				</Suspense>
			)}
			{copyTarget && (
				<Suspense fallback={null}>
					<BatchTargetFolderDialog
						open={true}
						onOpenChange={(open) => {
							if (!open) setCopyTarget(null);
						}}
						mode="copy"
						onConfirm={handleCopyConfirm}
						currentFolderId={folderId}
						initialBreadcrumb={breadcrumb}
						selectedFolderIds={
							copyTarget.type === "folder" ? [copyTarget.id] : []
						}
					/>
				</Suspense>
			)}
			{moveTarget && (
				<Suspense fallback={null}>
					<BatchTargetFolderDialog
						open={true}
						onOpenChange={(open) => {
							if (!open) setMoveTarget(null);
						}}
						mode="move"
						onConfirm={handleMoveConfirm}
						currentFolderId={folderId}
						initialBreadcrumb={breadcrumb}
						selectedFolderIds={moveTarget.folderIds}
					/>
				</Suspense>
			)}
			{versionTarget && (
				<Suspense fallback={null}>
					<VersionHistoryDialog
						open={true}
						onOpenChange={(open) => {
							if (!open) setVersionTarget(null);
						}}
						fileId={versionTarget.fileId}
						fileName={versionTarget.fileName}
						mimeType={versionTarget.mimeType}
						currentSize={versionTarget.size}
						onRestored={() => {
							setVersionTarget(null);
							refresh();
						}}
					/>
				</Suspense>
			)}
			{renameTarget && (
				<Suspense fallback={null}>
					<RenameDialog
						open={true}
						onOpenChange={(open) => {
							if (!open) setRenameTarget(null);
						}}
						type={renameTarget.type}
						id={renameTarget.id}
						currentName={renameTarget.name}
					/>
				</Suspense>
			)}
			<Suspense fallback={null}>
				<FileInfoDialog
					open={infoTarget !== null}
					onOpenChange={(open) => {
						if (!open) setInfoTarget(null);
					}}
					file={infoTarget?.file}
					folder={infoTarget?.folder}
				/>
			</Suspense>
		</AppLayout>
	);
}
