import { FilePreviewDialog } from "@/components/files/preview/FilePreviewDialog";
import type {
	ArchivePreviewManifest,
	FileInfo,
	FileListItem,
	PreviewLinkInfo,
	ShareStreamSessionInfo,
	WopiLaunchSession,
} from "@/types/api";

interface FilePreviewProps {
	file: FileInfo | FileListItem;
	onClose: () => void;
	onOpenChangeComplete?: (open: boolean) => void;
	onFileUpdated?: () => void;
	downloadPath?: string;
	editable?: boolean;
	previewLinkFactory?: () => Promise<PreviewLinkInfo>;
	archivePreviewFactory?: (options?: {
		signal?: AbortSignal;
	}) => Promise<ArchivePreviewManifest>;
	mediaStreamLinkFactory?: () => Promise<ShareStreamSessionInfo>;
	wopiSessionFactory?: (appKey: string) => Promise<WopiLaunchSession>;
	open?: boolean;
	openMode?: "auto" | "direct" | "picker";
}

export function FilePreview({
	file,
	onClose,
	onOpenChangeComplete,
	onFileUpdated,
	downloadPath,
	editable,
	previewLinkFactory,
	archivePreviewFactory,
	mediaStreamLinkFactory,
	wopiSessionFactory,
	open = true,
	openMode,
}: FilePreviewProps) {
	return (
		<FilePreviewDialog
			open={open}
			file={file}
			onClose={onClose}
			onOpenChangeComplete={onOpenChangeComplete}
			onFileUpdated={onFileUpdated}
			downloadPath={downloadPath}
			editable={editable}
			previewLinkFactory={previewLinkFactory}
			archivePreviewFactory={archivePreviewFactory}
			mediaStreamLinkFactory={mediaStreamLinkFactory}
			wopiSessionFactory={wopiSessionFactory}
			openMode={openMode}
		/>
	);
}
