import { FilePreviewDialog } from "@/components/files/preview/FilePreviewDialog";
import type { FileInfo, FileListItem, PreviewLinkInfo } from "@/types/api";

interface FilePreviewProps {
	file: FileInfo | FileListItem;
	onClose: () => void;
	onFileUpdated?: () => void;
	downloadPath?: string;
	editable?: boolean;
	previewLinkFactory?: () => Promise<PreviewLinkInfo>;
}

export function FilePreview({
	file,
	onClose,
	onFileUpdated,
	downloadPath,
	editable,
	previewLinkFactory,
}: FilePreviewProps) {
	return (
		<FilePreviewDialog
			file={file}
			onClose={onClose}
			onFileUpdated={onFileUpdated}
			downloadPath={downloadPath}
			editable={editable}
			previewLinkFactory={previewLinkFactory}
		/>
	);
}
