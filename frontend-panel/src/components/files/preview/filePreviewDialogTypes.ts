import type { MusicPlayerTrack } from "@/stores/musicPlayerStore";
import type {
	ArchiveFilenameEncoding,
	ArchivePreviewManifest,
	FileInfo,
	FileListItem,
	PreviewLinkInfo,
	ShareStreamSessionInfo,
	WopiLaunchSession,
} from "@/types/api";

export interface FilePreviewDialogProps {
	open: boolean;
	file: FileInfo | FileListItem;
	onClose: () => void;
	onOpenChangeComplete?: (open: boolean) => void;
	onFileUpdated?: () => void;
	downloadPath?: string;
	imagePreviewPath?: string;
	thumbnailPath?: string;
	editable?: boolean;
	previewLinkFactory?: () => Promise<PreviewLinkInfo>;
	archivePreviewFactory?: (options?: {
		signal?: AbortSignal;
		filenameEncoding?: ArchiveFilenameEncoding;
	}) => Promise<ArchivePreviewManifest>;
	loadMusicBackendMetadata?: MusicPlayerTrack["loadBackendMetadata"];
	mediaStreamLinkFactory?: () => Promise<ShareStreamSessionInfo>;
	wopiSessionFactory?: (appKey: string) => Promise<WopiLaunchSession>;
	openMode?: "auto" | "direct" | "picker";
}
