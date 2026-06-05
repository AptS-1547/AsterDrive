import type { CSSProperties, Ref } from "react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { useBlobUrl } from "@/hooks/useBlobUrl";
import { useFrontendConfigStore } from "@/stores/frontendConfigStore";
import { PreviewError } from "./PreviewError";
import { PreviewLoadingState } from "./PreviewLoadingState";
import type { PreviewableFileLike } from "./types";

interface BlobImagePreviewProps {
	file: PreviewableFileLike;
	fallbackPath?: string;
	fillContainer?: boolean;
	imageClassName?: string;
	imageRef?: Ref<HTMLImageElement>;
	imageStyle?: CSSProperties;
	onShowOriginalStateChange?: (state: ShowOriginalState) => void;
	onSourceChange?: (source: ImagePreviewSource) => void;
	path: string;
	showOriginalRequestId?: number;
	showOriginalButtonPlacement?: "inline" | "none";
	viewportClassName?: string;
	viewportRef?: Ref<HTMLDivElement>;
}

function isSvgPreview(file: PreviewableFileLike) {
	return (
		file.mime_type.toLowerCase() === "image/svg+xml" ||
		file.name.toLowerCase().endsWith(".svg")
	);
}

export type ImagePreviewSource = "original" | "backend_preview";
export type ShowOriginalState = "hidden" | "available" | "loading" | "success";

export function BlobImagePreview({
	file,
	fallbackPath,
	fillContainer = false,
	imageClassName: imageClassNameProp,
	imageRef,
	imageStyle,
	onShowOriginalStateChange,
	onSourceChange,
	path,
	showOriginalButtonPlacement = "inline",
	showOriginalRequestId,
	viewportClassName,
	viewportRef,
}: BlobImagePreviewProps) {
	const { t } = useTranslation("files");
	const imagePreviewPreference = useFrontendConfigStore(
		(state) => state.imagePreviewPreference,
	);
	const previewKey = `${file.name}\u0000${file.mime_type}\u0000${path}\u0000${
		fallbackPath ?? ""
	}\u0000${imagePreviewPreference}`;
	const [requestedOriginalKey, setRequestedOriginalKey] = useState<
		string | null
	>(null);
	const [displayOriginalKey, setDisplayOriginalKey] = useState<string | null>(
		null,
	);
	const canShowOriginal =
		imagePreviewPreference === "preview_first" && fallbackPath != null;
	const activeSource: ImagePreviewSource =
		canShowOriginal && displayOriginalKey !== previewKey
			? "backend_preview"
			: "original";
	const activePath: string | null =
		activeSource === "backend_preview" ? (fallbackPath ?? null) : path;
	const { blobUrl, error, loading, retry } = useBlobUrl(activePath, {
		lane: activeSource === "backend_preview" ? "preview" : "default",
	});
	const isLoadingOriginal =
		canShowOriginal &&
		requestedOriginalKey === previewKey &&
		displayOriginalKey !== previewKey;
	const {
		blobUrl: originalBlobUrl,
		error: originalError,
		loading: originalLoading,
	} = useBlobUrl(isLoadingOriginal ? path : null, {
		lane: "default",
	});
	const [imageRenderFailedKey, setImageRenderFailedKey] = useState<
		string | null
	>(null);
	const imageRenderFailed =
		imageRenderFailedKey === `${previewKey}\u0000${activeSource}`;
	const canRequestOriginal =
		canShowOriginal && activeSource === "backend_preview";
	const showOriginalState: ShowOriginalState = canRequestOriginal
		? isLoadingOriginal
			? "loading"
			: "available"
		: canShowOriginal && displayOriginalKey === previewKey
			? "success"
			: "hidden";
	const imageContainerClass =
		viewportClassName ??
		(fillContainer
			? "flex h-full min-h-0 w-full items-center justify-center p-4"
			: isSvgPreview(file)
				? "flex w-full items-center justify-center p-4"
				: "mx-auto flex w-fit max-w-full min-w-0 items-center justify-center p-4");
	const imageClass =
		imageClassNameProp ??
		(fillContainer
			? "block h-full w-full min-w-0 object-contain"
			: isSvgPreview(file)
				? "block h-auto w-full max-h-[min(70vh,48rem)] max-w-[min(70vw,48rem)] min-w-0 object-contain"
				: "block max-h-[min(70vh,48rem)] max-w-full min-w-0 object-contain");

	const handleImageError = () => {
		setImageRenderFailedKey(`${previewKey}\u0000${activeSource}`);
	};

	const handleRetry = () => {
		setImageRenderFailedKey(null);
		retry();
	};

	const handleShowOriginal = () => {
		setImageRenderFailedKey(null);
		setRequestedOriginalKey(previewKey);
	};

	useEffect(() => {
		onSourceChange?.(activeSource);
	}, [activeSource, onSourceChange]);

	useEffect(() => {
		onShowOriginalStateChange?.(showOriginalState);
	}, [onShowOriginalStateChange, showOriginalState]);

	useEffect(() => {
		if (showOriginalRequestId == null || !canRequestOriginal) return;
		setImageRenderFailedKey(null);
		setRequestedOriginalKey(previewKey);
	}, [canRequestOriginal, previewKey, showOriginalRequestId]);

	useEffect(() => {
		if (
			!isLoadingOriginal ||
			originalLoading ||
			originalError ||
			!originalBlobUrl
		) {
			return;
		}
		setImageRenderFailedKey(null);
		setDisplayOriginalKey(previewKey);
	}, [
		isLoadingOriginal,
		originalBlobUrl,
		originalError,
		originalLoading,
		previewKey,
	]);

	useEffect(() => {
		if (requestedOriginalKey !== previewKey) return;
		if (originalError) {
			setRequestedOriginalKey(null);
		}
	}, [originalError, previewKey, requestedOriginalKey]);

	const originalButton =
		canRequestOriginal && showOriginalButtonPlacement === "inline" ? (
			<Button
				type="button"
				variant="outline"
				size="sm"
				className="shrink-0"
				onClick={handleShowOriginal}
				disabled={isLoadingOriginal}
			>
				<Icon
					name={isLoadingOriginal ? "Spinner" : "Eye"}
					className={`mr-1.5 size-4 ${isLoadingOriginal ? "animate-spin" : ""}`}
				/>
				{t("preview_show_original")}
			</Button>
		) : null;

	const content = loading ? (
		<PreviewLoadingState text={t("loading_preview")} className="h-full" />
	) : error || !blobUrl || imageRenderFailed ? (
		<PreviewError onRetry={handleRetry} />
	) : (
		<div ref={viewportRef} className={imageContainerClass}>
			<img
				ref={imageRef}
				src={blobUrl}
				alt={file.name}
				draggable={false}
				onError={handleImageError}
				className={imageClass}
				style={imageStyle}
			/>
		</div>
	);

	if (!originalButton) {
		return content;
	}

	return (
		<div
			className={
				fillContainer
					? "flex h-full min-h-0 w-full flex-col"
					: "flex w-full flex-col"
			}
		>
			<div className="flex justify-end px-4 pt-4">{originalButton}</div>
			<div className={fillContainer ? "min-h-0 flex-1" : undefined}>
				{content}
			</div>
		</div>
	);
}
