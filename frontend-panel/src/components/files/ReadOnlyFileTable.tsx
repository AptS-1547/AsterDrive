import { useTranslation } from "react-i18next";
import {
	FileNameCell,
	FileSizeCell,
	FolderNameCell,
	FolderSizeCell,
	UpdatedAtCell,
} from "@/components/files/FileTableCells";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import type { FileListItem, FolderListItem } from "@/types/api";

interface ReadOnlyFileTableProps {
	folders: FolderListItem[];
	files: FileListItem[];
	onFolderClick?: (folder: FolderListItem) => void;
	onFileClick?: (file: FileListItem) => void;
	onFileDownload?: (file: FileListItem) => void;
	getThumbnailPath?: (file: FileListItem) => string;
}

export function ReadOnlyFileTable({
	folders,
	files,
	onFolderClick,
	onFileClick,
	onFileDownload,
	getThumbnailPath,
}: ReadOnlyFileTableProps) {
	const { t } = useTranslation(["files", "common"]);

	return (
		<Table>
			<TableHeader>
				<TableRow>
					<TableHead>{t("common:name")}</TableHead>
					<TableHead className="w-[100px]">{t("common:size")}</TableHead>
					<TableHead>{t("common:date")}</TableHead>
					<TableHead className="w-[64px]" />
				</TableRow>
			</TableHeader>
			<TableBody>
				{folders.map((folder) => (
					<TableRow
						key={`folder-${folder.id}`}
						className="cursor-pointer"
						onClick={() => onFolderClick?.(folder)}
					>
						<FolderNameCell folder={folder} />
						<FolderSizeCell />
						<UpdatedAtCell updatedAt={folder.updated_at} />
						<TableCell />
					</TableRow>
				))}
				{files.map((file) => (
					<TableRow
						key={`file-${file.id}`}
						className="cursor-pointer"
						onClick={() => onFileClick?.(file)}
					>
						<FileNameCell
							file={file}
							thumbnailPath={getThumbnailPath?.(file)}
						/>
						<FileSizeCell size={file.size} />
						<UpdatedAtCell updatedAt={file.updated_at} />
						<TableCell className="pr-2 text-right">
							{onFileDownload ? (
								<Button
									variant="ghost"
									size="icon-sm"
									onClick={(e) => {
										e.stopPropagation();
										onFileDownload(file);
									}}
								>
									<Icon name="Download" className="h-4 w-4" />
								</Button>
							) : null}
						</TableCell>
					</TableRow>
				))}
			</TableBody>
		</Table>
	);
}
