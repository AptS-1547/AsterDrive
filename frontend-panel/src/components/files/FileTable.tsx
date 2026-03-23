import type React from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { FileContextMenu } from "@/components/files/FileContextMenu";
import {
	FileNameCell,
	FileSizeCell,
	FolderNameCell,
	FolderSizeCell,
	UpdatedAtCell,
} from "@/components/files/FileTableCells";
import { Icon } from "@/components/ui/icon";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { cn } from "@/lib/utils";
import type { SortBy } from "@/stores/fileStore";
import { useFileStore } from "@/stores/fileStore";
import type { FileInfo, FolderInfo } from "@/types/api";

interface FileTableProps {
	folders: FolderInfo[];
	files: FileInfo[];
	onFolderOpen: (id: number, name: string) => void;
	onFileClick: (file: FileInfo) => void;
	onShare: (target: {
		fileId?: number;
		folderId?: number;
		name: string;
	}) => void;
	onDownload: (fileId: number, fileName: string) => void;
	onCopy: (type: "file" | "folder", id: number) => void;
	onMove?: (type: "file" | "folder", id: number) => void;
	onToggleLock: (type: "file" | "folder", id: number, locked: boolean) => void;
	onDelete: (type: "file" | "folder", id: number) => void;
	onVersions?: (fileId: number) => void;
	onMoveToFolder?: (
		fileIds: number[],
		folderIds: number[],
		targetFolderId: number,
	) => void;
	fadingFileIds?: Set<number>;
	fadingFolderIds?: Set<number>;
}

function SortIcon({
	column,
	current,
	order,
}: {
	column: SortBy;
	current: SortBy;
	order: "asc" | "desc";
}) {
	if (column !== current) return null;
	return order === "asc" ? (
		<Icon name="ArrowUp" className="h-3 w-3 ml-1" />
	) : (
		<Icon name="ArrowDown" className="h-3 w-3 ml-1" />
	);
}

export function FileTable({
	folders,
	files,
	onFolderOpen,
	onFileClick,
	onShare,
	onDownload,
	onCopy,
	onMove,
	onToggleLock,
	onDelete,
	onVersions,
	onMoveToFolder,
	fadingFileIds,
	fadingFolderIds,
}: FileTableProps) {
	const { t } = useTranslation("files");
	const selectedFileIds = useFileStore((s) => s.selectedFileIds);
	const selectedFolderIds = useFileStore((s) => s.selectedFolderIds);
	const toggleFileSelection = useFileStore((s) => s.toggleFileSelection);
	const toggleFolderSelection = useFileStore((s) => s.toggleFolderSelection);
	const selectAll = useFileStore((s) => s.selectAll);
	const clearSelection = useFileStore((s) => s.clearSelection);
	const sortBy = useFileStore((s) => s.sortBy);
	const sortOrder = useFileStore((s) => s.sortOrder);
	const setSortBy = useFileStore((s) => s.setSortBy);
	const toggleSortOrder = useFileStore((s) => s.toggleSortOrder);

	const allSelected =
		folders.length + files.length > 0 &&
		selectedFileIds.size === files.length &&
		selectedFolderIds.size === folders.length;

	const handleSort = (col: SortBy) => {
		if (sortBy === col) {
			toggleSortOrder();
		} else {
			setSortBy(col);
		}
	};

	const handleSelectAll = () => {
		if (allSelected) clearSelection();
		else selectAll();
	};

	const DRAG_MIME = "application/x-asterdrive-move";
	const [dragOverId, setDragOverId] = useState<number | null>(null);

	const makeDragData = (itemId: number, isFolder: boolean) => {
		const isSelected = isFolder
			? selectedFolderIds.has(itemId)
			: selectedFileIds.has(itemId);
		if (isSelected && selectedFileIds.size + selectedFolderIds.size > 1) {
			return {
				fileIds: [...selectedFileIds],
				folderIds: [...selectedFolderIds],
			};
		}
		return isFolder
			? { fileIds: [], folderIds: [itemId] }
			: { fileIds: [itemId], folderIds: [] };
	};

	const handleDragStart = (
		e: React.DragEvent,
		itemId: number,
		isFolder: boolean,
	) => {
		e.dataTransfer.setData(
			DRAG_MIME,
			JSON.stringify(makeDragData(itemId, isFolder)),
		);
		e.dataTransfer.effectAllowed = "move";
	};

	const handleFolderDragOver = (e: React.DragEvent, folderId: number) => {
		if (!e.dataTransfer.types.includes(DRAG_MIME)) return;
		e.preventDefault();
		e.dataTransfer.dropEffect = "move";
		setDragOverId(folderId);
	};

	const handleFolderDrop = (e: React.DragEvent, folderId: number) => {
		setDragOverId(null);
		e.preventDefault();
		const raw = e.dataTransfer.getData(DRAG_MIME);
		if (!raw) return;
		const data = JSON.parse(raw) as { fileIds: number[]; folderIds: number[] };
		if (data.folderIds.includes(folderId)) return;
		onMoveToFolder?.(data.fileIds, data.folderIds, folderId);
	};

	return (
		<Table>
			<TableHeader>
				<TableRow>
					<TableHead className="w-8 px-1">
						{/* biome-ignore lint/a11y/useSemanticElements: custom styled checkbox */}
						<div
							className={cn(
								"h-4 w-4 rounded border flex items-center justify-center cursor-pointer",
								allSelected
									? "bg-primary border-primary"
									: "border-muted-foreground",
							)}
							onClick={handleSelectAll}
							onKeyDown={() => {}}
							role="checkbox"
							aria-checked={allSelected}
							tabIndex={0}
						>
							{allSelected && (
								// biome-ignore lint/a11y/noSvgWithoutTitle: decorative checkmark
								<svg
									viewBox="0 0 12 12"
									className="h-3 w-3 text-primary-foreground"
									fill="none"
									stroke="currentColor"
									strokeWidth="2"
								>
									<polyline points="2,6 5,9 10,3" />
								</svg>
							)}
						</div>
					</TableHead>
					<TableHead
						className="cursor-pointer select-none"
						onClick={() => handleSort("name")}
					>
						<div className="flex items-center">
							{t("common:name")}
							<SortIcon column="name" current={sortBy} order={sortOrder} />
						</div>
					</TableHead>
					<TableHead
						className="w-[100px] cursor-pointer select-none"
						onClick={() => handleSort("size")}
					>
						<div className="flex items-center">
							{t("common:size")}
							<SortIcon column="size" current={sortBy} order={sortOrder} />
						</div>
					</TableHead>
					<TableHead
						className="cursor-pointer select-none"
						onClick={() => handleSort("date")}
					>
						<div className="flex items-center">
							{t("common:date")}
							<SortIcon column="date" current={sortBy} order={sortOrder} />
						</div>
					</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{folders.map((folder) => (
					<FileContextMenu
						renderTrigger
						key={`folder-${folder.id}`}
						isFolder
						isLocked={folder.is_locked ?? false}
						onShare={() =>
							onShare({
								folderId: folder.id,
								name: folder.name,
							})
						}
						onCopy={() => onCopy("folder", folder.id)}
						onMove={onMove ? () => onMove("folder", folder.id) : undefined}
						onToggleLock={() =>
							onToggleLock("folder", folder.id, folder.is_locked ?? false)
						}
						onDelete={() => onDelete("folder", folder.id)}
					>
						<TableRow
							className={cn(
								"cursor-pointer transition-all duration-300",
								dragOverId === folder.id && "ring-2 ring-primary bg-accent/30",
								fadingFolderIds?.has(folder.id) && "opacity-0 scale-95",
							)}
							draggable
							onDragStart={(e) => handleDragStart(e, folder.id, true)}
							onDragOver={(e) => handleFolderDragOver(e, folder.id)}
							onDragLeave={() => setDragOverId(null)}
							onDrop={(e) => handleFolderDrop(e, folder.id)}
							onClick={() => onFolderOpen(folder.id, folder.name)}
						>
							<TableCell className="px-1" onClick={(e) => e.stopPropagation()}>
								{/* biome-ignore lint/a11y/useSemanticElements: custom styled checkbox */}
								<div
									className={cn(
										"h-4 w-4 rounded border flex items-center justify-center cursor-pointer",
										selectedFolderIds.has(folder.id)
											? "bg-primary border-primary"
											: "border-muted-foreground/50",
									)}
									onClick={() => toggleFolderSelection(folder.id)}
									onKeyDown={() => {}}
									role="checkbox"
									aria-checked={selectedFolderIds.has(folder.id)}
									tabIndex={-1}
								>
									{selectedFolderIds.has(folder.id) && (
										// biome-ignore lint/a11y/noSvgWithoutTitle: decorative checkmark
										<svg
											viewBox="0 0 12 12"
											className="h-3 w-3 text-primary-foreground"
											fill="none"
											stroke="currentColor"
											strokeWidth="2"
										>
											<polyline points="2,6 5,9 10,3" />
										</svg>
									)}
								</div>
							</TableCell>
							<FolderNameCell folder={folder} />
							<FolderSizeCell />
							<UpdatedAtCell updatedAt={folder.updated_at} />
						</TableRow>
					</FileContextMenu>
				))}
				{files.map((file) => (
					<FileContextMenu
						renderTrigger
						key={`file-${file.id}`}
						isFolder={false}
						isLocked={file.is_locked ?? false}
						onDownload={() => onDownload(file.id, file.name)}
						onShare={() => onShare({ fileId: file.id, name: file.name })}
						onCopy={() => onCopy("file", file.id)}
						onMove={onMove ? () => onMove("file", file.id) : undefined}
						onToggleLock={() =>
							onToggleLock("file", file.id, file.is_locked ?? false)
						}
						onDelete={() => onDelete("file", file.id)}
						onVersions={onVersions ? () => onVersions(file.id) : undefined}
					>
						<TableRow
							className={cn(
								"cursor-pointer transition-all duration-300",
								fadingFileIds?.has(file.id) && "opacity-0 scale-95",
							)}
							draggable
							onDragStart={(e) => handleDragStart(e, file.id, false)}
							onClick={() => onFileClick(file)}
						>
							<TableCell className="px-1" onClick={(e) => e.stopPropagation()}>
								{/* biome-ignore lint/a11y/useSemanticElements: custom styled checkbox */}
								<div
									className={cn(
										"h-4 w-4 rounded border flex items-center justify-center cursor-pointer",
										selectedFileIds.has(file.id)
											? "bg-primary border-primary"
											: "border-muted-foreground/50",
									)}
									onClick={() => toggleFileSelection(file.id)}
									onKeyDown={() => {}}
									role="checkbox"
									aria-checked={selectedFileIds.has(file.id)}
									tabIndex={-1}
								>
									{selectedFileIds.has(file.id) && (
										// biome-ignore lint/a11y/noSvgWithoutTitle: decorative checkmark
										<svg
											viewBox="0 0 12 12"
											className="h-3 w-3 text-primary-foreground"
											fill="none"
											stroke="currentColor"
											strokeWidth="2"
										>
											<polyline points="2,6 5,9 10,3" />
										</svg>
									)}
								</div>
							</TableCell>
							<FileNameCell file={file} />
							<FileSizeCell size={file.size} />
							<UpdatedAtCell updatedAt={file.updated_at} />
						</TableRow>
					</FileContextMenu>
				))}
			</TableBody>
		</Table>
	);
}
