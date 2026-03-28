import { useVirtualizer } from "@tanstack/react-virtual";
import type React from "react";
import { useEffect, useMemo, useState } from "react";
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
import { ItemCheckbox } from "@/components/ui/item-checkbox";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { DRAG_SOURCE_MIME } from "@/lib/constants";
import {
	getInvalidInternalDropReason,
	hasInternalDragData,
	readInternalDragData,
	setInternalDragPreview,
	writeInternalDragData,
} from "@/lib/dragDrop";
import { cn } from "@/lib/utils";
import type { SortBy } from "@/stores/fileStore";
import { useFileStore } from "@/stores/fileStore";
import type { FileListItem, FolderListItem } from "@/types/api";

interface FileTableProps {
	folders: FolderListItem[];
	files: FileListItem[];
	scrollElement?: HTMLDivElement | null;
	breadcrumbPathIds?: number[];
	onFolderOpen: (id: number, name: string) => void;
	onFileClick: (file: FileListItem) => void;
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
	onRename?: (type: "file" | "folder", id: number, name: string) => void;
	onVersions?: (fileId: number) => void;
	onInfo?: (type: "file" | "folder", id: number) => void;
	onMoveToFolder?: (
		fileIds: number[],
		folderIds: number[],
		targetFolderId: number,
	) => void;
	fadingFileIds?: Set<number>;
	fadingFolderIds?: Set<number>;
}

type TableRowItem =
	| { type: "folder"; item: FolderListItem }
	| { type: "file"; item: FileListItem };

const TABLE_COLUMN_COUNT = 4;
const TABLE_ROW_ESTIMATE = 52;

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
		<Icon name="SortAscending" className="h-3 w-3 ml-1" />
	) : (
		<Icon name="SortDescending" className="h-3 w-3 ml-1" />
	);
}

export function FileTable({
	folders,
	files,
	scrollElement,
	breadcrumbPathIds = [],
	onFolderOpen,
	onFileClick,
	onShare,
	onDownload,
	onCopy,
	onMove,
	onToggleLock,
	onDelete,
	onRename,
	onVersions,
	onInfo,
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
	const setSortOrder = useFileStore((s) => s.setSortOrder);

	const allSelected =
		folders.length + files.length > 0 &&
		selectedFileIds.size === files.length &&
		selectedFolderIds.size === folders.length;

	const handleSort = (col: SortBy) => {
		if (sortBy === col) {
			setSortOrder(sortOrder === "asc" ? "desc" : "asc");
		} else {
			setSortBy(col);
		}
	};

	const handleSelectAll = () => {
		if (allSelected) clearSelection();
		else selectAll();
	};

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
		const data = makeDragData(itemId, isFolder);
		writeInternalDragData(e.dataTransfer, data);
		setInternalDragPreview(e, {
			variant: "list-row",
			itemCount: data.fileIds.length + data.folderIds.length,
		});
	};

	const getTargetPathIds = (folderId: number) => [
		...breadcrumbPathIds,
		folderId,
	];

	const handleFolderDragOver = (e: React.DragEvent, folderId: number) => {
		if (
			!hasInternalDragData(e.dataTransfer) ||
			e.dataTransfer.types.includes(DRAG_SOURCE_MIME)
		) {
			return;
		}
		e.preventDefault();
		e.stopPropagation();
		e.dataTransfer.dropEffect = "move";
		setDragOverId(folderId);
	};

	const handleFolderDrop = (e: React.DragEvent, folderId: number) => {
		setDragOverId(null);
		if (e.dataTransfer.types.includes(DRAG_SOURCE_MIME)) {
			return;
		}
		e.preventDefault();
		e.stopPropagation();
		const data = readInternalDragData(e.dataTransfer);
		if (!data) return;
		const targetPathIds = getTargetPathIds(folderId);
		if (getInvalidInternalDropReason(data, folderId, targetPathIds) !== null) {
			return;
		}
		onMoveToFolder?.(data.fileIds, data.folderIds, folderId);
	};

	const renderFolderRow = (folder: FolderListItem) => (
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
			onRename={
				onRename ? () => onRename("folder", folder.id, folder.name) : undefined
			}
			onToggleLock={() =>
				onToggleLock("folder", folder.id, folder.is_locked ?? false)
			}
			onDelete={() => onDelete("folder", folder.id)}
			onInfo={() => onInfo?.("folder", folder.id)}
		>
			<TableRow
				data-folder-drop-target="true"
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
				<TableCell
					className="w-12 pl-3 pr-0"
					onClick={(e) => e.stopPropagation()}
				>
					<div className="flex justify-center">
						<ItemCheckbox
							checked={selectedFolderIds.has(folder.id)}
							onChange={() => toggleFolderSelection(folder.id)}
						/>
					</div>
				</TableCell>
				<FolderNameCell folder={folder} />
				<FolderSizeCell />
				<UpdatedAtCell updatedAt={folder.updated_at} />
			</TableRow>
		</FileContextMenu>
	);

	const renderFileRow = (file: FileListItem) => (
		<FileContextMenu
			renderTrigger
			key={`file-${file.id}`}
			isFolder={false}
			isLocked={file.is_locked ?? false}
			onDownload={() => onDownload(file.id, file.name)}
			onShare={() => onShare({ fileId: file.id, name: file.name })}
			onCopy={() => onCopy("file", file.id)}
			onMove={onMove ? () => onMove("file", file.id) : undefined}
			onRename={
				onRename ? () => onRename("file", file.id, file.name) : undefined
			}
			onToggleLock={() =>
				onToggleLock("file", file.id, file.is_locked ?? false)
			}
			onDelete={() => onDelete("file", file.id)}
			onVersions={onVersions ? () => onVersions(file.id) : undefined}
			onInfo={() => onInfo?.("file", file.id)}
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
				<TableCell
					className="w-12 pl-3 pr-0"
					onClick={(e) => e.stopPropagation()}
				>
					<div className="flex justify-center">
						<ItemCheckbox
							checked={selectedFileIds.has(file.id)}
							onChange={() => toggleFileSelection(file.id)}
						/>
					</div>
				</TableCell>
				<FileNameCell file={file} />
				<FileSizeCell size={file.size} />
				<UpdatedAtCell updatedAt={file.updated_at} />
			</TableRow>
		</FileContextMenu>
	);

	const tableRows = useMemo<TableRowItem[]>(
		() => [
			...folders.map((item) => ({ type: "folder", item }) as const),
			...files.map((item) => ({ type: "file", item }) as const),
		],
		[files, folders],
	);

	const virtualizer = useVirtualizer({
		count: scrollElement ? tableRows.length : 0,
		getScrollElement: () => scrollElement ?? null,
		estimateSize: () => TABLE_ROW_ESTIMATE,
		overscan: 10,
	});

	useEffect(() => {
		if (!scrollElement) return;
		virtualizer.measure();
	}, [scrollElement, virtualizer]);

	const renderSpacerRow = (key: string, height: number) => (
		<TableRow key={key} aria-hidden className="border-0 hover:bg-transparent">
			<TableCell
				colSpan={TABLE_COLUMN_COUNT}
				className="p-0"
				style={{ height }}
			/>
		</TableRow>
	);

	const renderRows = () => {
		if (!scrollElement) {
			return (
				<>
					{folders.map(renderFolderRow)}
					{files.map(renderFileRow)}
				</>
			);
		}

		const virtualRows = virtualizer.getVirtualItems();
		const firstVirtualRow = virtualRows[0];
		const lastVirtualRow = virtualRows[virtualRows.length - 1];
		const paddingTop = firstVirtualRow?.start ?? 0;
		const paddingBottom = Math.max(
			0,
			virtualizer.getTotalSize() - (lastVirtualRow?.end ?? 0),
		);

		return (
			<>
				{paddingTop > 0 && renderSpacerRow("spacer-top", paddingTop)}
				{virtualRows.map((virtualRow) => {
					const row = tableRows[virtualRow.index];
					if (!row) return null;
					return row.type === "folder"
						? renderFolderRow(row.item)
						: renderFileRow(row.item);
				})}
				{paddingBottom > 0 && renderSpacerRow("spacer-bottom", paddingBottom)}
			</>
		);
	};

	return (
		<Table>
			<TableHeader>
				<TableRow>
					<TableHead className="w-12 pl-3 pr-0">
						<div className="flex justify-center">
							<ItemCheckbox checked={allSelected} onChange={handleSelectAll} />
						</div>
					</TableHead>
					<TableHead
						className="cursor-pointer select-none"
						onClick={() => handleSort("name")}
					>
						<div className="flex items-center">
							{t("name")}
							<SortIcon column="name" current={sortBy} order={sortOrder} />
						</div>
					</TableHead>
					<TableHead
						className="w-[100px] cursor-pointer select-none"
						onClick={() => handleSort("size")}
					>
						<div className="flex items-center">
							{t("size")}
							<SortIcon column="size" current={sortBy} order={sortOrder} />
						</div>
					</TableHead>
					<TableHead
						className="cursor-pointer select-none"
						onClick={() => handleSort("created_at")}
					>
						<div className="flex items-center">
							{t("date")}
							<SortIcon
								column="created_at"
								current={sortBy}
								order={sortOrder}
							/>
						</div>
					</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>{renderRows()}</TableBody>
		</Table>
	);
}
