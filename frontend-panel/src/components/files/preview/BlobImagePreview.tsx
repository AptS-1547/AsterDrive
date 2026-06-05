import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useBlobUrl } from "@/hooks/useBlobUrl";
import {
	getThumbnailExtension,
	supportsImagePreviewExtension,
} from "@/lib/thumbnailSupport";
import { useThumbnailSupportStore } from "@/stores/thumbnailSupportStore";
import { PreviewError } from "./PreviewError";
import { PreviewLoadingState } from "./PreviewLoadingState";
import type { PreviewableFileLike } from "./types";

interface BlobImagePreviewProps {
	file: PreviewableFileLike;
	fallbackPath?: string;
	fillContainer?: boolean;
	path: string;
}

const BROWSER_NATIVE_IMAGE_EXTENSIONS = new Set([
	"apng",
	"avif",
	"bmp",
	"gif",
	"ico",
	"jfif",
	"jpe",
	"jpeg",
	"jpg",
	"png",
	"svg",
	"webp",
]);

function isSvgPreview(file: PreviewableFileLike) {
	return (
		file.mime_type.toLowerCase() === "image/svg+xml" ||
		file.name.toLowerCase().endsWith(".svg")
	);
}

function shouldUseBackendImagePreview(
	file: PreviewableFileLike,
	extensions: string[] | undefined,
) {
	const extensionCandidates = [getThumbnailExtension(file.name)].filter(
		Boolean,
	);
	const mime = file.mime_type.trim().toLowerCase();
	if (mime === "image/heic") {
		extensionCandidates.push("heic");
	}
	if (mime === "image/heif") {
		extensionCandidates.push("heif");
	}

	return extensionCandidates.some(
		(extension) =>
			!BROWSER_NATIVE_IMAGE_EXTENSIONS.has(extension) &&
			supportsImagePreviewExtension(`preview.${extension}`, extensions),
	);
}

export function BlobImagePreview({
	file,
	fallbackPath,
	fillContainer = false,
	path,
}: BlobImagePreviewProps) {
	const { t } = useTranslation("files");
	const imagePreviewExtensions = useThumbnailSupportStore(
		(state) => state.config?.image_preview?.extensions,
	);
	const previewKey = `${file.name}\u0000${file.mime_type}\u0000${path}\u0000${
		fallbackPath ?? ""
	}`;
	const [imageRenderFailedKey, setImageRenderFailedKey] = useState<
		string | null
	>(null);
	const shouldPreferBackendPreview =
		Boolean(fallbackPath) &&
		shouldUseBackendImagePreview(file, imagePreviewExtensions);
	const imageRenderFailed = imageRenderFailedKey === previewKey;
	const activePath = shouldPreferBackendPreview ? (fallbackPath ?? path) : path;
	const { blobUrl, error, loading, retry } = useBlobUrl(activePath);

	const handleImageError = () => {
		setImageRenderFailedKey(previewKey);
	};

	const handleRetry = () => {
		setImageRenderFailedKey(null);
		retry();
	};

	if (loading) {
		return (
			<PreviewLoadingState text={t("loading_preview")} className="h-full" />
		);
	}

	if (error || !blobUrl || imageRenderFailed) {
		return <PreviewError onRetry={handleRetry} />;
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
				onError={handleImageError}
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
