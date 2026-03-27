import { FileItemStatusIndicators } from "@/components/files/FileItemStatusIndicators";
import { FileThumbnail } from "@/components/files/FileThumbnail";
import { Icon } from "@/components/ui/icon";
import { TableCell } from "@/components/ui/table";
import { formatBytes, formatDate } from "@/lib/format";
import type { FileListItem, FolderListItem } from "@/types/api";

export function FileNameCell({
	file,
	thumbnailPath,
	isLocked,
}: {
	file: FileListItem;
	thumbnailPath?: string;
	isLocked?: boolean;
}) {
	return (
		<TableCell className="pl-1 pr-2">
			<div className="flex min-w-0 items-center gap-2.5">
				<FileThumbnail file={file} size="sm" thumbnailPath={thumbnailPath} />
				<div className="flex min-w-0 items-center gap-2">
					<span className="min-w-0 truncate" title={file.name}>
						{file.name}
					</span>
					<FileItemStatusIndicators
						isShared={file.is_shared}
						isLocked={file.is_locked}
						compact
					/>
				</div>
			</div>
		</TableCell>
	);
}

export function FolderNameCell({ folder }: { folder: FolderListItem }) {
	return (
		<TableCell className="pl-1 pr-2">
			<div className="flex min-w-0 items-center gap-2.5">
				<Icon name="Folder" className="h-4 w-4 shrink-0 text-amber-500" />
				<div className="flex min-w-0 items-center gap-2">
					<span className="min-w-0 truncate" title={folder.name}>
						{folder.name}
					</span>
					<FileItemStatusIndicators
						isShared={folder.is_shared}
						isLocked={folder.is_locked}
						compact
					/>
				</div>
			</div>
		</TableCell>
	);
}

export function FileSizeCell({ size }: { size: number }) {
	return (
		<TableCell className="text-muted-foreground">{formatBytes(size)}</TableCell>
	);
}

export function FolderSizeCell() {
	return <TableCell className="text-muted-foreground">---</TableCell>;
}

export function UpdatedAtCell({ updatedAt }: { updatedAt: string }) {
	return (
		<TableCell className="text-muted-foreground">
			{formatDate(updatedAt)}
		</TableCell>
	);
}
