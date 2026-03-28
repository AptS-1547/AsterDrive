import { useBlobUrl } from "@/hooks/useBlobUrl";
import { fileService } from "@/services/fileService";
import type { FileInfo, FileListItem } from "@/types/api";
import { FileTypeIcon } from "./FileTypeIcon";

interface FileThumbnailProps {
	file: FileInfo | FileListItem;
	size?: "sm" | "lg";
	thumbnailPath?: string;
}

export function FileThumbnail({
	file,
	size = "sm",
	thumbnailPath,
}: FileThumbnailProps) {
	const isImage =
		file.mime_type.startsWith("image/") && file.mime_type !== "image/svg+xml";
	const { blobUrl, error } = useBlobUrl(
		isImage ? (thumbnailPath ?? fileService.thumbnailPath(file.id)) : null,
	);

	if (size === "sm") {
		return (
			<div className="flex h-6 w-6 shrink-0 items-center justify-center overflow-hidden rounded-md bg-muted/40">
				{!isImage || error || !blobUrl ? (
					<FileTypeIcon
						mimeType={file.mime_type}
						fileName={file.name}
						className="h-4 w-4"
					/>
				) : (
					<img
						src={blobUrl}
						alt=""
						loading="lazy"
						decoding="async"
						draggable={false}
						className="h-full w-full object-cover"
					/>
				)}
			</div>
		);
	}

	if (!isImage || error || !blobUrl) {
		return (
			<FileTypeIcon
				mimeType={file.mime_type}
				fileName={file.name}
				className="h-12 w-12"
			/>
		);
	}

	return (
		<img
			src={blobUrl}
			alt=""
			loading="lazy"
			decoding="async"
			draggable={false}
			className="h-full w-auto shrink-0 max-w-none"
		/>
	);
}
