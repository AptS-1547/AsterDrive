import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileCard } from "@/components/files/FileCard";
import { FileContextMenu } from "@/components/files/FileContextMenu";
import { useFileStore } from "@/stores/fileStore";
import type { FileListItem, FolderListItem } from "@/types/api";

interface FileGridProps {
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

const GRID_CLASSES =
	"grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-3";
const GRID_HEADER_BOTTOM_GAP = 8;
const GRID_SECTION_TOP_GAP = 16;
const GRID_ROW_GAP = 12;
const GRID_HEADER_ESTIMATE = 28;
const GRID_ITEM_ROW_ESTIMATE = 176;

type GridItem =
	| { type: "folder"; item: FolderListItem }
	| { type: "file"; item: FileListItem };

type GridRow =
	| {
			type: "section-header";
			key: string;
			label: string;
			paddingTop: number;
	  }
	| {
			type: "items";
			key: string;
			items: GridItem[];
			paddingBottom: number;
	  };

function getGridColumnCount(viewportWidth: number) {
	if (viewportWidth >= 1280) return 6;
	if (viewportWidth >= 1024) return 5;
	if (viewportWidth >= 768) return 4;
	if (viewportWidth >= 640) return 3;
	return 2;
}

export function FileGrid({
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
	onRename,
	onDelete,
	onVersions,
	onInfo,
	onMoveToFolder,
	fadingFileIds,
	fadingFolderIds,
}: FileGridProps) {
	const { t } = useTranslation("files");
	const selectedFileIds = useFileStore((s) => s.selectedFileIds);
	const selectedFolderIds = useFileStore((s) => s.selectedFolderIds);
	const toggleFileSelection = useFileStore((s) => s.toggleFileSelection);
	const toggleFolderSelection = useFileStore((s) => s.toggleFolderSelection);
	const [viewportWidth, setViewportWidth] = useState(() =>
		typeof window === "undefined" ? 1280 : window.innerWidth,
	);

	const hasBoth = folders.length > 0 && files.length > 0;

	useEffect(() => {
		if (typeof window === "undefined") return;

		const updateViewportWidth = () => {
			setViewportWidth(window.innerWidth);
		};

		updateViewportWidth();
		window.addEventListener("resize", updateViewportWidth);
		return () => window.removeEventListener("resize", updateViewportWidth);
	}, []);

	const getDragData = (itemId: number, isFolder: boolean) => {
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

	const getTargetPathIds = (folderId: number) => [
		...breadcrumbPathIds,
		folderId,
	];

	const renderFolderCard = (folder: FolderListItem) => (
		<FileContextMenu
			key={`folder-${folder.id}`}
			isFolder
			isLocked={folder.is_locked ?? false}
			onShare={() => onShare({ folderId: folder.id, name: folder.name })}
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
			<FileCard
				item={folder}
				isFolder
				selected={selectedFolderIds.has(folder.id)}
				onSelect={() => toggleFolderSelection(folder.id)}
				onClick={() => onFolderOpen(folder.id, folder.name)}
				dragData={getDragData(folder.id, true)}
				onDrop={onMoveToFolder}
				targetPathIds={getTargetPathIds(folder.id)}
				fading={fadingFolderIds?.has(folder.id)}
			/>
		</FileContextMenu>
	);

	const renderFileCard = (file: FileListItem) => (
		<FileContextMenu
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
			<FileCard
				item={file}
				isFolder={false}
				selected={selectedFileIds.has(file.id)}
				onSelect={() => toggleFileSelection(file.id)}
				onClick={() => onFileClick(file)}
				dragData={getDragData(file.id, false)}
				fading={fadingFileIds?.has(file.id)}
			/>
		</FileContextMenu>
	);

	const columnCount = getGridColumnCount(viewportWidth);
	const gridRows = useMemo(() => {
		const rows: GridRow[] = [];

		const appendSectionRows = (
			type: GridItem["type"],
			items: FolderListItem[] | FileListItem[],
			label: string,
		) => {
			if (items.length === 0) return;

			if (hasBoth) {
				rows.push({
					type: "section-header",
					key: `${type}-header`,
					label,
					paddingTop: rows.length === 0 ? 0 : GRID_SECTION_TOP_GAP,
				});
			}

			for (let index = 0; index < items.length; index += columnCount) {
				const slice = items.slice(index, index + columnCount);
				rows.push({
					type: "items",
					key: `${type}-row-${slice[0]?.id ?? index}`,
					items: slice.map((item) => ({ type, item })) as GridItem[],
					paddingBottom: index + columnCount < items.length ? GRID_ROW_GAP : 0,
				});
			}
		};

		appendSectionRows("folder", folders, t("folders_section"));
		appendSectionRows("file", files, t("files_section"));

		return rows;
	}, [columnCount, files, folders, hasBoth, t]);

	const virtualizer = useVirtualizer({
		count: scrollElement ? gridRows.length : 0,
		getScrollElement: () => scrollElement ?? null,
		estimateSize: (index) =>
			gridRows[index]?.type === "section-header"
				? GRID_HEADER_ESTIMATE
				: GRID_ITEM_ROW_ESTIMATE,
		overscan: 4,
	});

	useEffect(() => {
		if (!scrollElement) return;
		virtualizer.measure();
	}, [scrollElement, virtualizer]);

	if (scrollElement) {
		const virtualRows = virtualizer.getVirtualItems();
		const firstVirtualRow = virtualRows[0];
		const lastVirtualRow = virtualRows[virtualRows.length - 1];
		const paddingTop = firstVirtualRow?.start ?? 0;
		const paddingBottom = Math.max(
			0,
			virtualizer.getTotalSize() - (lastVirtualRow?.end ?? 0),
		);

		return (
			<div className="p-4">
				{paddingTop > 0 && <div aria-hidden style={{ height: paddingTop }} />}
				{virtualRows.map((virtualRow) => {
					const row = gridRows[virtualRow.index];
					if (!row) return null;
					if (row.type === "section-header") {
						return (
							<h3
								key={row.key}
								ref={virtualizer.measureElement}
								data-index={virtualRow.index}
								className="px-1 text-sm font-medium text-muted-foreground"
								style={{
									paddingTop: row.paddingTop,
									paddingBottom: GRID_HEADER_BOTTOM_GAP,
								}}
							>
								{row.label}
							</h3>
						);
					}

					return (
						<div
							key={row.key}
							ref={virtualizer.measureElement}
							data-index={virtualRow.index}
							className={GRID_CLASSES}
							style={{ paddingBottom: row.paddingBottom }}
						>
							{row.items.map((item) =>
								item.type === "folder"
									? renderFolderCard(item.item)
									: renderFileCard(item.item),
							)}
						</div>
					);
				})}
				{paddingBottom > 0 && (
					<div aria-hidden style={{ height: paddingBottom }} />
				)}
			</div>
		);
	}

	return (
		<div className="p-4 space-y-4">
			{folders.length > 0 && (
				<div className="space-y-2">
					{hasBoth && (
						<h3 className="text-sm font-medium text-muted-foreground px-1">
							{t("folders_section")}
						</h3>
					)}
					<div className={GRID_CLASSES}>{folders.map(renderFolderCard)}</div>
				</div>
			)}

			{files.length > 0 && (
				<div className="space-y-2">
					{hasBoth && (
						<h3 className="text-sm font-medium text-muted-foreground px-1">
							{t("files_section")}
						</h3>
					)}
					<div className={GRID_CLASSES}>{files.map(renderFileCard)}</div>
				</div>
			)}
		</div>
	);
}
