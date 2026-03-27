import { EmptyState } from "@/components/common/EmptyState";
import { ReadOnlyFileGrid } from "@/components/files/ReadOnlyFileGrid";
import { ReadOnlyFileTable } from "@/components/files/ReadOnlyFileTable";
import { Icon } from "@/components/ui/icon";
import type { FileListItem, FolderListItem } from "@/types/api";

interface ReadOnlyFileCollectionProps {
	folders: FolderListItem[];
	files: FileListItem[];
	viewMode: "grid" | "list";
	onFolderClick?: (folder: FolderListItem) => void;
	onFileClick?: (file: FileListItem) => void;
	onFileDownload?: (file: FileListItem) => void;
	getThumbnailPath?: (file: FileListItem) => string;
	emptyTitle: string;
	emptyDescription: string;
}

export function ReadOnlyFileCollection({
	folders,
	files,
	viewMode,
	onFolderClick,
	onFileClick,
	onFileDownload,
	getThumbnailPath,
	emptyTitle,
	emptyDescription,
}: ReadOnlyFileCollectionProps) {
	const isEmpty = folders.length === 0 && files.length === 0;

	return (
		<div className="min-h-0 flex-1 overflow-auto">
			{isEmpty ? (
				<EmptyState
					icon={<Icon name="FolderOpen" className="h-12 w-12" />}
					title={emptyTitle}
					description={emptyDescription}
				/>
			) : viewMode === "grid" ? (
				<ReadOnlyFileGrid
					folders={folders}
					files={files}
					onFolderClick={onFolderClick}
					onFileClick={onFileClick}
					onFileDownload={onFileDownload}
					getThumbnailPath={getThumbnailPath}
				/>
			) : (
				<ReadOnlyFileTable
					folders={folders}
					files={files}
					onFolderClick={onFolderClick}
					onFileClick={onFileClick}
					onFileDownload={onFileDownload}
					getThumbnailPath={getThumbnailPath}
				/>
			)}
		</div>
	);
}
