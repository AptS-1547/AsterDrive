import Artplayer from "artplayer";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useBlobUrl } from "@/hooks/useBlobUrl";
import { logger } from "@/lib/logger";
import { PreviewError } from "./PreviewError";
import { PreviewLoadingState } from "./PreviewLoadingState";
import type { PreviewableFileLike } from "./types";

const DEFAULT_ASPECT_RATIO = 16 / 9;
const DIALOG_CHROME_HEIGHT_REM = 11;

interface VideoPreviewProps {
	file: PreviewableFileLike;
	path: string;
}

function getPlayerLanguage(language: string) {
	return language.startsWith("zh") ? "zh-cn" : "en";
}

export function VideoPreview({ file, path }: VideoPreviewProps) {
	const { t, i18n } = useTranslation("files");
	const containerRef = useRef<HTMLDivElement | null>(null);
	const { blobUrl, error, loading, retry } = useBlobUrl(path);
	const [playerFailed, setPlayerFailed] = useState(false);
	const [aspectRatio, setAspectRatio] = useState(DEFAULT_ASPECT_RATIO);

	const playerLanguage = useMemo(
		() => getPlayerLanguage(i18n.language),
		[i18n.language],
	);
	const previewFrameStyle = useMemo(
		() => ({
			aspectRatio: String(aspectRatio),
			maxWidth: `min(100%, calc((90vh - ${DIALOG_CHROME_HEIGHT_REM}rem) * ${aspectRatio}))`,
		}),
		[aspectRatio],
	);

	useEffect(() => {
		setPlayerFailed(false);
		setAspectRatio(DEFAULT_ASPECT_RATIO);
		if (!blobUrl) return;

		const metadataVideo = document.createElement("video");

		const handleLoadedMetadata = () => {
			if (metadataVideo.videoWidth <= 0 || metadataVideo.videoHeight <= 0)
				return;
			setAspectRatio(metadataVideo.videoWidth / metadataVideo.videoHeight);
		};

		metadataVideo.preload = "metadata";
		metadataVideo.src = blobUrl;
		metadataVideo.addEventListener("loadedmetadata", handleLoadedMetadata);
		metadataVideo.load();

		return () => {
			metadataVideo.removeEventListener("loadedmetadata", handleLoadedMetadata);
			metadataVideo.removeAttribute("src");
			metadataVideo.load();
		};
	}, [blobUrl]);

	useEffect(() => {
		if (!blobUrl || !containerRef.current || playerFailed) return;

		let art: Artplayer | null = null;

		try {
			art = new Artplayer({
				container: containerRef.current,
				url: blobUrl,
				lang: playerLanguage,
				fullscreen: true,
				fullscreenWeb: true,
				pip: true,
				setting: true,
				playbackRate: true,
				miniProgressBar: true,
				mutex: true,
				hotkey: true,
				playsInline: true,
				airplay: true,
			});
			art.template.$video.style.objectFit = "contain";
		} catch (playerError) {
			logger.warn("artplayer init failed", file.name, playerError);
			setPlayerFailed(true);
		}

		return () => {
			art?.destroy(false);
		};
	}, [blobUrl, file.name, playerFailed, playerLanguage]);

	if (loading) {
		return (
			<PreviewLoadingState text={t("loading_preview")} className="h-full" />
		);
	}

	if (error || !blobUrl) {
		return <PreviewError onRetry={retry} />;
	}

	if (playerFailed) {
		return (
			<div
				className="mx-auto w-full overflow-hidden rounded-xl bg-black"
				style={previewFrameStyle}
			>
				{/* biome-ignore lint/a11y/useMediaCaption: user-uploaded media may not have captions available */}
				<video
					src={blobUrl}
					controls
					className="block h-full w-full object-contain"
				/>
			</div>
		);
	}

	return (
		<div
			className="mx-auto w-full overflow-hidden rounded-xl bg-black"
			style={previewFrameStyle}
		>
			<div ref={containerRef} className="h-full w-full" />
		</div>
	);
}
