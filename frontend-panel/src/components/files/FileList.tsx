import {
	Copy,
	Download,
	FileIcon,
	Folder,
	Link,
	Loader2,
	Lock,
	Trash2,
	Unlock,
} from "lucide-react";
import { useState } from "react";
import type { FileInfo } from "@/types/api";
import { toast } from "sonner";
import { FilePreview } from "@/components/files/FilePreview";
import { ShareDialog } from "@/components/files/ShareDialog";
import { VersionHistoryDialog } from "@/components/files/VersionHistoryDialog";
import { Button } from "@/components/ui/button";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { fileService } from "@/services/fileService";
import { useFileStore } from "@/stores/fileStore";

function FileThumbnail({ file }: { file: FileInfo }) {
	const [failed, setFailed] = useState(false);
	const isImage =
		file.mime_type.startsWith("image/") && file.mime_type !== "image/svg+xml";

	if (!isImage || failed) {
		return <FileIcon className="h-4 w-4 text-muted-foreground" />;
	}

	return (
		<img
			src={fileService.thumbnailUrl(file.id)}
			alt=""
			className="h-8 w-8 rounded object-cover"
			loading="lazy"
			onError={() => setFailed(true)}
		/>
	);
}

function formatDate(dateStr: string): string {
	const date = new Date(dateStr);
	const now = new Date();
	const diff = now.getTime() - date.getTime();
	const minutes = Math.floor(diff / 60000);
	if (minutes < 1) return "just now";
	if (minutes < 60) return `${minutes}m ago`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours}h ago`;
	const days = Math.floor(hours / 24);
	if (days < 30) return `${days}d ago`;
	return date.toLocaleDateString();
}

export function FileList() {
	const folders = useFileStore((s) => s.folders);
	const files = useFileStore((s) => s.files);
	const navigateTo = useFileStore((s) => s.navigateTo);
	const deleteFile = useFileStore((s) => s.deleteFile);
	const deleteFolder = useFileStore((s) => s.deleteFolder);
	const loading = useFileStore((s) => s.loading);
	const refresh = useFileStore((s) => s.refresh);

	const handleToggleLock = async (
		type: "file" | "folder",
		id: number,
		currentLocked: boolean,
	) => {
		try {
			if (type === "file") {
				await fileService.setFileLock(id, !currentLocked);
			} else {
				await fileService.setFolderLock(id, !currentLocked);
			}
			toast.success(currentLocked ? "Unlocked" : "Locked");
			refresh();
		} catch (error) {
			handleApiError(error);
		}
	};

	const handleCopyFile = async (id: number) => {
		try {
			await fileService.copyFile(id);
			toast.success("File copied");
			refresh();
		} catch (error) {
			handleApiError(error);
		}
	};

	const handleCopyFolder = async (id: number) => {
		try {
			await fileService.copyFolder(id);
			toast.success("Folder copied");
			refresh();
		} catch (error) {
			handleApiError(error);
		}
	};

	const [shareTarget, setShareTarget] = useState<{
		fileId?: number;
		folderId?: number;
		name: string;
	} | null>(null);
	const [downloadingId, setDownloadingId] = useState<number | null>(null);
	const [previewFile, setPreviewFile] = useState<FileInfo | null>(null);

	const handleDownload = async (fileId: number, fileName: string) => {
		setDownloadingId(fileId);
		try {
			const url = fileService.downloadUrl(fileId);
			const res = await fetch(url, { credentials: "include" });
			if (!res.ok) throw new Error(`download failed: ${res.status}`);
			const blob = await res.blob();
			const objectUrl = URL.createObjectURL(blob);
			const a = document.createElement("a");
			a.href = objectUrl;
			a.download = fileName;
			a.click();
			URL.revokeObjectURL(objectUrl);
		} catch (err) {
			handleApiError(err);
		} finally {
			setDownloadingId(null);
		}
	};

	const handleDeleteFile = async (id: number) => {
		try {
			await deleteFile(id);
			toast.success("File deleted");
		} catch (error) {
			handleApiError(error);
		}
	};

	const handleDeleteFolder = async (id: number) => {
		try {
			await deleteFolder(id);
			toast.success("Folder deleted");
		} catch (error) {
			handleApiError(error);
		}
	};

	if (loading) {
		return (
			<div className="flex-1 flex items-center justify-center text-muted-foreground">
				Loading...
			</div>
		);
	}

	if (folders.length === 0 && files.length === 0) {
		return (
			<div className="flex-1 flex items-center justify-center text-muted-foreground">
				This folder is empty
			</div>
		);
	}

	return (
		<>
			<Table>
				<TableHeader>
					<TableRow>
						<TableHead className="w-[50%]">Name</TableHead>
						<TableHead>Modified</TableHead>
						<TableHead className="w-[120px]">Actions</TableHead>
					</TableRow>
				</TableHeader>
				<TableBody>
					{folders.map((folder) => (
						<TableRow
							key={`folder-${folder.id}`}
							className="cursor-pointer"
							onClick={() => navigateTo(folder.id, folder.name)}
						>
							<TableCell className="flex items-center gap-2">
								<Folder className="h-4 w-4 text-blue-500" />
								{folder.name}
							</TableCell>
							<TableCell className="text-muted-foreground">
								{formatDate(folder.updated_at)}
							</TableCell>
							<TableCell>
								<div className="flex gap-1">
									<Button
										variant="ghost"
										size="icon"
										className="h-8 w-8"
										onClick={(e) => {
											e.stopPropagation();
											setShareTarget({
												folderId: folder.id,
												name: folder.name,
											});
										}}
									>
										<Link className="h-4 w-4" />
									</Button>
									<Button
										variant="ghost"
										size="icon"
										className="h-8 w-8"
										onClick={(e) => {
											e.stopPropagation();
											handleCopyFolder(folder.id);
										}}
									>
										<Copy className="h-4 w-4" />
									</Button>
									<Button
										variant="ghost"
										size="icon"
										className="h-8 w-8"
										onClick={(e) => {
											e.stopPropagation();
											handleToggleLock(
												"folder",
												folder.id,
												folder.is_locked ?? false,
											);
										}}
									>
										{folder.is_locked ? (
											<Lock className="h-4 w-4 text-red-500" />
										) : (
											<Unlock className="h-4 w-4 text-muted-foreground" />
										)}
									</Button>
									<Button
										variant="ghost"
										size="icon"
										className="h-8 w-8"
										disabled={folder.is_locked}
										onClick={(e) => {
											e.stopPropagation();
											handleDeleteFolder(folder.id);
										}}
									>
										<Trash2 className="h-4 w-4" />
									</Button>
								</div>
							</TableCell>
						</TableRow>
					))}
					{files.map((file) => (
						<TableRow key={`file-${file.id}`}>
							<TableCell
								className="flex items-center gap-2 cursor-pointer hover:text-blue-600"
								onClick={() => setPreviewFile(file)}
							>
								<FileThumbnail file={file} />
								{file.name}
							</TableCell>
							<TableCell className="text-muted-foreground">
								{formatDate(file.updated_at)}
							</TableCell>
							<TableCell>
								<div className="flex gap-1">
									<Button
										variant="ghost"
										size="icon"
										className="h-8 w-8"
										onClick={() =>
											setShareTarget({
												fileId: file.id,
												name: file.name,
											})
										}
									>
										<Link className="h-4 w-4" />
									</Button>
									<Button
										variant="ghost"
										size="icon"
										className="h-8 w-8"
										disabled={downloadingId === file.id}
										onClick={() => handleDownload(file.id, file.name)}
									>
										{downloadingId === file.id ? (
											<Loader2 className="h-4 w-4 animate-spin" />
										) : (
											<Download className="h-4 w-4" />
										)}
									</Button>
									<Button
										variant="ghost"
										size="icon"
										className="h-8 w-8"
										onClick={() => handleCopyFile(file.id)}
									>
										<Copy className="h-4 w-4" />
									</Button>
									<VersionHistoryDialog
										fileId={file.id}
										fileName={file.name}
										onRestored={() => refresh()}
									/>
									<Button
										variant="ghost"
										size="icon"
										className="h-8 w-8"
										onClick={() =>
											handleToggleLock("file", file.id, file.is_locked ?? false)
										}
									>
										{file.is_locked ? (
											<Lock className="h-4 w-4 text-red-500" />
										) : (
											<Unlock className="h-4 w-4 text-muted-foreground" />
										)}
									</Button>
									<Button
										variant="ghost"
										size="icon"
										className="h-8 w-8"
										disabled={file.is_locked}
										onClick={() => handleDeleteFile(file.id)}
									>
										<Trash2 className="h-4 w-4" />
									</Button>
								</div>
							</TableCell>
						</TableRow>
					))}
				</TableBody>
			</Table>
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
		</>
	);
}
