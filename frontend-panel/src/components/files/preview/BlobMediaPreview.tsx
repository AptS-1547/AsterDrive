import { useTranslation } from "react-i18next";
import { useBlobUrl } from "@/hooks/useBlobUrl";
import { PreviewError } from "./PreviewError";
import type { PreviewableFileLike } from "./types";

interface BlobMediaPreviewProps {
	file: PreviewableFileLike;
	mode: "image" | "video" | "audio";
	path: string;
}

export function BlobMediaPreview({ file, mode, path }: BlobMediaPreviewProps) {
	const { t } = useTranslation("files");
	const { blobUrl, error, loading, retry } = useBlobUrl(path);

	if (loading) {
		return (
			<div className="p-6 text-sm text-muted-foreground">
				{t("loading_preview")}
			</div>
		);
	}

	if (error || !blobUrl) {
		return <PreviewError onRetry={retry} />;
	}

	if (mode === "image") {
		return (
			<img
				src={blobUrl}
				alt={file.name}
				className="max-w-full max-h-full object-contain mx-auto"
			/>
		);
	}

	if (mode === "video") {
		return (
			// biome-ignore lint/a11y/useMediaCaption: user-uploaded media may not have captions available
			<video src={blobUrl} controls className="max-w-full max-h-full mx-auto" />
		);
	}

	if (mode === "audio") {
		return (
			<div className="flex min-h-[50vh] items-center justify-center px-6">
				{/* biome-ignore lint/a11y/useMediaCaption: user-uploaded media may not have captions available */}
				<audio src={blobUrl} controls className="w-full max-w-3xl" />
			</div>
		);
	}

	return null;
}
