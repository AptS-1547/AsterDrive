import { useTranslation } from "react-i18next";
import { useBlobUrl } from "@/hooks/useBlobUrl";
import { PreviewError } from "./PreviewError";
import { PreviewLoadingState } from "./PreviewLoadingState";
import type { PreviewableFileLike } from "./types";

interface BlobMediaPreviewProps {
	file: PreviewableFileLike;
	fillContainer?: boolean;
	mode: "image" | "video" | "audio";
	path: string;
}

function isSvgPreview(file: PreviewableFileLike) {
	return (
		file.mime_type.toLowerCase() === "image/svg+xml" ||
		file.name.toLowerCase().endsWith(".svg")
	);
}

export function BlobMediaPreview({
	file,
	fillContainer = false,
	mode,
	path,
}: BlobMediaPreviewProps) {
	const { t } = useTranslation("files");
	const { blobUrl, error, loading, retry } = useBlobUrl(path);

	if (loading) {
		return (
			<PreviewLoadingState text={t("loading_preview")} className="h-full" />
		);
	}

	if (error || !blobUrl) {
		return <PreviewError onRetry={retry} />;
	}

	if (mode === "image") {
		const isSvg = isSvgPreview(file);

		return (
			<div
				className={
					fillContainer
						? "flex h-full min-h-0 w-full items-center justify-center p-4"
						: isSvg
							? "flex w-full items-center justify-center p-4"
							: "mx-auto flex w-fit max-w-full min-w-0 items-center justify-center p-4"
				}
			>
				<img
					src={blobUrl}
					alt={file.name}
					className={
						fillContainer
							? "block h-full w-full min-w-0 object-contain"
							: isSvg
								? "block h-auto w-full max-h-[min(70vh,48rem)] max-w-[min(70vw,48rem)] min-w-0 object-contain"
								: "block max-h-[min(70vh,48rem)] max-w-full min-w-0 object-contain"
					}
				/>
			</div>
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
