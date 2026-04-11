import { FilePreviewDialog } from "@/components/files/preview/FilePreviewDialog";
import type {
	FileInfo,
	FileListItem,
	PreviewLinkInfo,
	WopiLaunchSession,
} from "@/types/api";

interface FilePreviewProps {
	file: FileInfo | FileListItem;
	onClose: () => void;
	onFileUpdated?: () => void;
	downloadPath?: string;
	editable?: boolean;
	previewLinkFactory?: () => Promise<PreviewLinkInfo>;
	wopiSessionFactory?: (appKey: string) => Promise<WopiLaunchSession>;
	openMode?: "auto" | "direct" | "picker";
}

export function FilePreview({
	file,
	onClose,
	onFileUpdated,
	downloadPath,
	editable,
	previewLinkFactory,
	wopiSessionFactory,
	openMode,
}: FilePreviewProps) {
	return (
		<FilePreviewDialog
			file={file}
			onClose={onClose}
			onFileUpdated={onFileUpdated}
			downloadPath={downloadPath}
			editable={editable}
			previewLinkFactory={previewLinkFactory}
			wopiSessionFactory={wopiSessionFactory}
			openMode={openMode}
		/>
	);
}
