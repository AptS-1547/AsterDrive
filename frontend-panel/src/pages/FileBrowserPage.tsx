import { Fragment, useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import { BatchActionBar } from "@/components/common/BatchActionBar";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonFileGrid } from "@/components/common/SkeletonFileGrid";
import { SkeletonFileTable } from "@/components/common/SkeletonFileTable";
import { ViewToggle } from "@/components/common/ViewToggle";
import { CreateFolderDialog } from "@/components/files/CreateFolderDialog";
import { FileGrid } from "@/components/files/FileGrid";
import { FilePreview } from "@/components/files/FilePreview";
import { FileTable } from "@/components/files/FileTable";
import { ShareDialog } from "@/components/files/ShareDialog";
import { UploadArea } from "@/components/files/UploadArea";
import { VersionHistoryDialog } from "@/components/files/VersionHistoryDialog";
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
import { formatBatchToast } from "@/lib/formatBatchToast";
import { fileService } from "@/services/fileService";
import { api } from "@/services/http";
import { useFileStore } from "@/stores/fileStore";
import type { FileInfo } from "@/types/api";

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

	const isSearching = searchQuery !== null;
	const displayFolders = isSearching ? searchFolders : folders;
	const displayFiles = isSearching ? searchFiles : files;

	useKeyboardShortcuts();

	const [createFolderOpen, setCreateFolderOpen] = useState(false);
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
	const [versionFileId, setVersionFileId] = useState<number | null>(null);

	useEffect(() => {
		navigateTo(folderId, folderName).catch(handleApiError);
	}, [folderId, folderName, navigateTo]);

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

	const handleCopy = useCallback(
		async (type: "file" | "folder", id: number) => {
			try {
				if (type === "file") await fileService.copyFile(id);
				else await fileService.copyFolder(id);
				toast.success(t("copy_success"));
				refresh();
			} catch (err) {
				handleApiError(err);
			}
		},
		[refresh, t],
	);

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
		async (fileIds: number[], folderIds: number[], targetFolderId: number) => {
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

	const sharedProps = {
		folders: displayFolders,
		files: displayFiles,
		onFolderOpen: (id: number, name: string) =>
			navigate(`/folder/${id}?name=${encodeURIComponent(name)}`),
		onFileClick: (file: FileInfo) => setPreviewFile(file),
		onShare: setShareTarget,
		onDownload: handleDownload,
		onCopy: handleCopy,
		onToggleLock: handleToggleLock,
		onDelete: handleDelete,
		onVersions: (fileId: number) => setVersionFileId(fileId),
		onMoveToFolder: handleMoveToFolder,
		fadingFileIds,
		fadingFolderIds,
	};

	const isEmpty =
		!loading && displayFolders.length === 0 && displayFiles.length === 0;

	return (
		<AppLayout actions={<ViewToggle value={viewMode} onChange={setViewMode} />}>
			<UploadArea>
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
					<ContextMenuTrigger className="flex-1 flex flex-col">
						<ScrollArea className="flex-1">
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
						</ScrollArea>
					</ContextMenuTrigger>
					<ContextMenuContent>
						<ContextMenuItem onClick={() => setCreateFolderOpen(true)}>
							<Icon name="FolderPlus" className="h-4 w-4 mr-2" />
							{t("new_folder")}
						</ContextMenuItem>
					</ContextMenuContent>
				</ContextMenu>
			</UploadArea>

			<CreateFolderDialog
				open={createFolderOpen}
				onOpenChange={setCreateFolderOpen}
			/>

			<BatchActionBar />

			{shareTarget && (
				<ShareDialog
					open={true}
					onOpenChange={(open) => {
						if (!open) setShareTarget(null);
					}}
					fileId={shareTarget.fileId}
					folderId={shareTarget.folderId}
					name={shareTarget.name}
				/>
			)}
			{previewFile && (
				<FilePreview file={previewFile} onClose={() => setPreviewFile(null)} />
			)}
			{versionFileId && (
				<VersionHistoryDialog
					fileId={versionFileId}
					fileName=""
					onRestored={() => {
						setVersionFileId(null);
						refresh();
					}}
				/>
			)}
		</AppLayout>
	);
}
