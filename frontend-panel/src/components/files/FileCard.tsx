import { useState } from "react";
import { FileThumbnail } from "@/components/files/FileThumbnail";
import { Icon } from "@/components/ui/icon";
import { cn } from "@/lib/utils";
import type { FileInfo, FolderInfo } from "@/types/api";

const DRAG_MIME = "application/x-asterdrive-move";

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
	) => void;
	fading?: boolean;
	draggable?: boolean;
}

export function FileCard({
	item,
	isFolder,
	selected,
	onSelect,
	onClick,
	dragData,
	onDrop,
	fading,
	draggable = true,
}: FileCardProps) {
	const [dragOver, setDragOver] = useState(false);

	const handleDragStart = (e: React.DragEvent) => {
		const data =
			dragData && (dragData.fileIds.length > 0 || dragData.folderIds.length > 0)
				? dragData
				: isFolder
					? { fileIds: [], folderIds: [item.id] }
					: { fileIds: [item.id], folderIds: [] };
		e.dataTransfer.setData(DRAG_MIME, JSON.stringify(data));
		e.dataTransfer.effectAllowed = "move";
	};

	const handleDragOver = (e: React.DragEvent) => {
		if (!isFolder || !e.dataTransfer.types.includes(DRAG_MIME)) return;
		e.preventDefault();
		e.dataTransfer.dropEffect = "move";
		setDragOver(true);
	};

	const handleDragLeave = () => setDragOver(false);

	const handleDrop = (e: React.DragEvent) => {
		setDragOver(false);
		if (!isFolder) return;
		e.preventDefault();
		const raw = e.dataTransfer.getData(DRAG_MIME);
		if (!raw) return;
		const data = JSON.parse(raw) as {
			fileIds: number[];
			folderIds: number[];
		};
		// Don't drop a folder into itself
		if (data.folderIds.includes(item.id)) return;
		onDrop?.(data.fileIds, data.folderIds, item.id);
	};

	return (
		// biome-ignore lint/a11y/useSemanticElements: card with nested interactive checkbox cannot be a button
		<div
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
			{/* biome-ignore lint/a11y/useSemanticElements: custom styled checkbox */}
			<div
				className={cn(
					"absolute top-2 left-2 h-4 w-4 rounded border flex items-center justify-center transition-opacity",
					selected
						? "opacity-100 bg-primary border-primary"
						: "opacity-0 group-hover:opacity-100 border-muted-foreground",
				)}
				onClick={(e) => {
					e.stopPropagation();
					onSelect();
				}}
				onKeyDown={() => {}}
				role="checkbox"
				aria-checked={selected}
				tabIndex={-1}
			>
				{selected && (
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

			{/* Icon / Thumbnail */}
			<div className="h-20 w-full flex items-center justify-center mb-2 rounded-lg bg-muted/40">
				{isFolder ? (
					<Icon name="Folder" className="h-12 w-12 text-amber-500" />
				) : (
					<FileThumbnail file={item as FileInfo} size="lg" />
				)}
			</div>

			{/* Name */}
			<span
				className="text-sm text-center w-full line-clamp-2 leading-tight"
				title={item.name}
			>
				{item.name}
			</span>
		</div>
	);
}
