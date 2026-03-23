import { EmptyState } from "@/components/common/EmptyState";
import { ViewToggle } from "@/components/common/ViewToggle";
import { ReadOnlyFileGrid } from "@/components/files/ReadOnlyFileGrid";
import { ReadOnlyFileTable } from "@/components/files/ReadOnlyFileTable";
import { Icon } from "@/components/ui/icon";
import type { FileInfo, FolderInfo } from "@/types/api";

interface ReadOnlyFileCollectionProps {
	folders: FolderInfo[];
	files: FileInfo[];
	viewMode: "grid" | "list";
	onViewModeChange: (mode: "grid" | "list") => void;
	onFolderClick?: (folder: FolderInfo) => void;
	onFileClick?: (file: FileInfo) => void;
	onFileDownload?: (file: FileInfo) => void;
	emptyTitle: string;
	emptyDescription: string;
}

export function ReadOnlyFileCollection({
	folders,
	files,
	viewMode,
	onViewModeChange,
	onFolderClick,
	onFileClick,
	onFileDownload,
	emptyTitle,
	emptyDescription,
}: ReadOnlyFileCollectionProps) {
	const isEmpty = folders.length === 0 && files.length === 0;

	return (
		<div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border bg-background shadow-sm">
			<div className="flex items-center justify-end border-b bg-muted/20 px-4 py-2">
				<ViewToggle value={viewMode} onChange={onViewModeChange} />
			</div>
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
					/>
				) : (
					<ReadOnlyFileTable
						folders={folders}
						files={files}
						onFolderClick={onFolderClick}
						onFileClick={onFileClick}
						onFileDownload={onFileDownload}
					/>
				)}
			</div>
		</div>
	);
}
