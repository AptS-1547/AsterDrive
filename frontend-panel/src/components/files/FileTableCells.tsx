import { FileThumbnail } from "@/components/files/FileThumbnail";
import { Icon } from "@/components/ui/icon";
import { TableCell } from "@/components/ui/table";
import { formatBytes, formatDate } from "@/lib/format";
import type { FileInfo, FolderInfo } from "@/types/api";

export function FileNameCell({
	file,
	thumbnailPath,
	isLocked,
}: {
	file: FileInfo;
	thumbnailPath?: string;
	isLocked?: boolean;
}) {
	return (
		<TableCell className="pl-1 pr-2">
			<div className="flex min-w-0 items-center gap-2.5">
				<FileThumbnail file={file} size="sm" thumbnailPath={thumbnailPath} />
				<span className="min-w-0 truncate" title={file.name}>
					{file.name}
				</span>
				{isLocked && (
					<Icon name="Lock" className="h-3 w-3 shrink-0 text-muted-foreground" />
				)}
			</div>
		</TableCell>
	);
}

export function FolderNameCell({
	folder,
	isLocked,
}: {
	folder: FolderInfo;
	isLocked?: boolean;
}) {
	return (
		<TableCell className="pl-1 pr-2">
			<div className="flex min-w-0 items-center gap-2.5">
				<Icon name="Folder" className="h-4 w-4 shrink-0 text-amber-500" />
				<span className="min-w-0 truncate" title={folder.name}>
					{folder.name}
				</span>
				{isLocked && (
					<Icon name="Lock" className="h-3 w-3 shrink-0 text-muted-foreground" />
				)}
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
