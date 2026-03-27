import { useState } from "react";
import { FileThumbnail } from "@/components/files/FileThumbnail";
import { Icon } from "@/components/ui/icon";
import { ItemCheckbox } from "@/components/ui/item-checkbox";
import {
	getInvalidInternalDropReason,
	hasInternalDragData,
	readInternalDragData,
	setInternalDragPreview,
	writeInternalDragData,
} from "@/lib/dragDrop";
import { cn } from "@/lib/utils";
import type { FileInfo, FolderInfo } from "@/types/api";

interface FileCardProps {
	item: FileInfo | FolderInfo;
	isFolder: boolean;
	selected: boolean;
	onSelect: () => void;
	onClick: () => void;
	/** IDs to drag when this item is part of a selection */
	dragData?: { fileIds: number[]; folderIds: number[] };
	onDrop?: (
		fileIds: number[],
		folderIds: number[],
		targetFolderId: number,
		targetPathIds: number[],
	) => void;
	targetPathIds?: number[];
	fading?: boolean;
	draggable?: boolean;
	thumbnailPath?: string;
}

export function FileCard({
	item,
	isFolder,
	selected,
	onSelect,
	onClick,
	dragData,
	onDrop,
	targetPathIds = [],
	fading,
	draggable = true,
	thumbnailPath,
}: FileCardProps) {
	const [dragOver, setDragOver] = useState(false);

	const handleDragStart = (e: React.DragEvent) => {
		const data =
			dragData && (dragData.fileIds.length > 0 || dragData.folderIds.length > 0)
				? dragData
				: isFolder
					? { fileIds: [], folderIds: [item.id] }
					: { fileIds: [item.id], folderIds: [] };
		writeInternalDragData(e.dataTransfer, data);
		setInternalDragPreview(e, {
			variant: "grid-card",
			itemCount: data.fileIds.length + data.folderIds.length,
		});
	};

	const handleDragOver = (e: React.DragEvent) => {
		if (!isFolder || !hasInternalDragData(e.dataTransfer)) return;
		e.preventDefault();
		e.dataTransfer.dropEffect = "move";
		setDragOver(true);
	};

	const handleDragLeave = () => setDragOver(false);

	const handleDrop = (e: React.DragEvent) => {
		setDragOver(false);
		if (!isFolder) return;
		e.preventDefault();
		const data = readInternalDragData(e.dataTransfer);
		if (!data) return;
		if (getInvalidInternalDropReason(data, item.id, targetPathIds) !== null) {
			return;
		}
		onDrop?.(data.fileIds, data.folderIds, item.id, targetPathIds);
	};

	return (
		// biome-ignore lint/a11y/useSemanticElements: card with nested interactive checkbox cannot be a button
		<div
			data-drag-preview-root
			className={cn(
				"group relative flex flex-col items-center p-3 rounded-lg border cursor-pointer transition-all duration-300 hover:bg-accent/50",
				selected && "bg-accent border-primary",
				draggable && dragOver && "ring-2 ring-primary bg-accent/30",
				fading && "opacity-0 scale-95",
			)}
			draggable={draggable}
			onDragStart={draggable ? handleDragStart : undefined}
			onDragOver={draggable ? handleDragOver : undefined}
			onDragLeave={draggable ? handleDragLeave : undefined}
			onDrop={draggable ? handleDrop : undefined}
			onClick={onClick}
			onKeyDown={(e) => e.key === "Enter" && onClick()}
			role="button"
			tabIndex={0}
		>
			<ItemCheckbox
				data-drag-preview-hidden
				checked={selected}
				onChange={onSelect}
				className={cn(
					"absolute top-2 left-2 transition-opacity",
					selected ? "opacity-100" : "opacity-0 group-hover:opacity-100",
				)}
			/>

			{/* Icon / Thumbnail */}
			<div
				data-drag-preview-media
				className="h-20 w-full flex items-center justify-center mb-2 rounded-lg bg-muted/40"
			>
				{isFolder ? (
					<Icon name="Folder" className="h-12 w-12 text-amber-500" />
				) : (
					<FileThumbnail
						file={item as FileInfo}
						size="lg"
						thumbnailPath={thumbnailPath}
					/>
				)}
			</div>

			{/* Name */}
			<span
				data-drag-preview-name
				className="text-sm text-center w-full line-clamp-2 leading-tight"
				title={item.name}
			>
				{item.name}
			</span>
		</div>
	);
}
