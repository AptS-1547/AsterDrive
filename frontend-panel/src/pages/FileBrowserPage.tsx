import {
	Fragment,
	Suspense,
	lazy,
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
import { ViewToggle } from "@/components/common/ViewToggle";
import { SortMenu } from "@/components/common/SortMenu";
import { FileGrid } from "@/components/files/FileGrid";
import { FileTable } from "@/components/files/FileTable";
import type { UploadAreaHandle } from "@/components/files/UploadArea";
import { AppLayout } from "@/components/layout/AppLayout";
import {
	Breadcrumb,
	BreadcrumbItem,
	BreadcrumbLink,
	BreadcrumbList,
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
import type { InternalDragData } from "@/lib/dragDrop";
import { formatBatchToast } from "@/lib/formatBatchToast";
import { batchService } from "@/services/batchService";
import { fileService } from "@/services/fileService";
import { api } from "@/services/http";
import { useAuthStore } from "@/stores/authStore";
import { useFileStore } from "@/stores/fileStore";
import type { FileInfo, FolderInfo } from "@/types/api";

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
	const [previewFile, setPreviewFile] = useState<FileInfo | null>(null);
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
	const [renameTarget, setRenameTarget] = useState<{
		type: "file" | "folder";
		id: number;
		name: string;
	} | null>(null);
	const [infoTarget, setInfoTarget] = useState<{
		file?: FileInfo;
		folder?: FolderInfo;
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

	const handleDownload = useCallback(
		async (fileId: number, fileName: string) => {
			try {
				const res = await api.client.get(fileService.downloadPath(fileId), {
					responseType: "blob",
				});
				const objectUrl = URL.createObjectURL(res.data);
				const a = document.createElement("a");
				a.href = objectUrl;
				a.download = fileName;
				a.click();
				URL.revokeObjectURL(objectUrl);
			} catch (err) {
				handleApiError(err);
			}
		},
		[],
	);

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

	const sharedProps = {
		folders: displayFolders,
		files: displayFiles,
		onFolderOpen: (id: number, name: string) =>
			navigate(`/folder/${id}?name=${encodeURIComponent(name)}`),
		onFileClick: (file: FileInfo) => setPreviewFile(file),
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
	const handleUploadAreaReady = useCallback((instance: UploadAreaHandle | null) => {
		uploadAreaRef.current = instance;
	}, []);
	const pageCore = (
		<>
			{/* Breadcrumb / search indicator */}
			<div className="px-4 pt-3 pb-1">
				{isSearching ? (
					<span className="text-sm text-muted-foreground">
						{t("common:search")}: &quot;{searchQuery}&quot;
					</span>
				) : (
					<Breadcrumb>
						<BreadcrumbList>
							{breadcrumb.map((item, i) => (
								<Fragment key={item.id ?? "root"}>
									{i > 0 && <BreadcrumbSeparator />}
									<BreadcrumbItem>
										{i < breadcrumb.length - 1 ? (
											<BreadcrumbLink
												className="cursor-pointer"
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
											<span className="font-medium">{item.name}</span>
										)}
									</BreadcrumbItem>
								</Fragment>
							))}
						</BreadcrumbList>
					</Breadcrumb>
				)}
			</div>
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
						<Icon name="Upload" className="h-4 w-4 mr-2" />
						{t("upload_file")}
					</ContextMenuItem>
					<ContextMenuItem
						disabled={!uploadReady}
						onClick={() => uploadAreaRef.current?.triggerFolderUpload()}
					>
						<Icon name="FolderOpen" className="h-4 w-4 mr-2" />
						{t("upload_folder")}
					</ContextMenuItem>
					<ContextMenuItem onClick={() => setCreateFolderOpen(true)}>
						<Icon name="FolderPlus" className="h-4 w-4 mr-2" />
						{t("new_folder")}
					</ContextMenuItem>
					<ContextMenuItem onClick={() => setCreateFileOpen(true)}>
						<Icon name="FilePlus" className="h-4 w-4 mr-2" />
						{t("new_file")}
					</ContextMenuItem>
				</ContextMenuContent>
			</ContextMenu>
		</>
	);

	return (
		<AppLayout
			actions={
				<div className="flex items-center gap-2">
					<SortMenu
						sortBy={sortBy}
						sortOrder={sortOrder}
						onSortBy={setSortBy}
						onSortOrder={setSortOrder}
					/>
					<ViewToggle value={viewMode} onChange={setViewMode} />
				</div>
			}
			onTrashDrop={handleTrashDrop}
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
