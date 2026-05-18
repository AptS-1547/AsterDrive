import { useTranslation } from "react-i18next";
import { useBlobUrl } from "@/hooks/useBlobUrl";
import { PreviewError } from "./PreviewError";
import { PreviewLoadingState } from "./PreviewLoadingState";
import type { PreviewableFileLike } from "./types";

interface BlobImagePreviewProps {
	file: PreviewableFileLike;
	fillContainer?: boolean;
	path: string;
}

function isSvgPreview(file: PreviewableFileLike) {
	return (
		file.mime_type.toLowerCase() === "image/svg+xml" ||
		file.name.toLowerCase().endsWith(".svg")
	);
}

export function BlobImagePreview({
	file,
	fillContainer = false,
	path,
}: BlobImagePreviewProps) {
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
