import { FilePreviewDialog } from "@/components/files/preview/FilePreviewDialog";
import type { FileInfo, FileListItem } from "@/types/api";

interface FilePreviewProps {
	file: FileInfo | FileListItem;
	onClose: () => void;
	onFileUpdated?: () => void;
	downloadPath?: string;
	editable?: boolean;
}

export function FilePreview({
	file,
	onClose,
	onFileUpdated,
	downloadPath,
	editable,
}: FilePreviewProps) {
	return (
		<FilePreviewDialog
			file={file}
			onClose={onClose}
			onFileUpdated={onFileUpdated}
			downloadPath={downloadPath}
			editable={editable}
		/>
	);
}
