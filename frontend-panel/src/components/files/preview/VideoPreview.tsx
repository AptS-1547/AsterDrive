import Artplayer from "artplayer";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { config } from "@/config/app";
import { joinApiUrl } from "@/lib/apiUrl";
import { logger } from "@/lib/logger";
import { PreviewError } from "./PreviewError";
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

function resolveVideoSource(path: string) {
	if (/^https?:\/\//i.test(path) || path.startsWith("blob:")) {
		return path;
	}
	if (
		path.startsWith("/api/") ||
		path.startsWith("/d/") ||
		path.startsWith("/pv/")
	) {
		return path;
	}
	return joinApiUrl(config.apiBaseUrl, path);
}

export function VideoPreview({ file, path }: VideoPreviewProps) {
	const { i18n } = useTranslation("files");
	const containerRef = useRef<HTMLDivElement | null>(null);
	const [playerFailed, setPlayerFailed] = useState(false);
	const [mediaFailed, setMediaFailed] = useState(false);
	const [aspectRatio, setAspectRatio] = useState(DEFAULT_ASPECT_RATIO);
	const videoSource = useMemo(() => resolveVideoSource(path), [path]);

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
		setMediaFailed(false);
		setAspectRatio(DEFAULT_ASPECT_RATIO);

		const metadataVideo = document.createElement("video");

		const handleLoadedMetadata = () => {
			if (metadataVideo.videoWidth <= 0 || metadataVideo.videoHeight <= 0)
				return;
			setAspectRatio(metadataVideo.videoWidth / metadataVideo.videoHeight);
		};

		metadataVideo.preload = "metadata";
		metadataVideo.src = videoSource;
		metadataVideo.addEventListener("loadedmetadata", handleLoadedMetadata);
		metadataVideo.load();

		return () => {
			metadataVideo.removeEventListener("loadedmetadata", handleLoadedMetadata);
			metadataVideo.removeAttribute("src");
			metadataVideo.load();
		};
	}, [videoSource]);

	useEffect(() => {
		if (!containerRef.current || playerFailed || mediaFailed) return;

		let art: Artplayer | null = null;
		let videoElement: HTMLVideoElement | null = null;
		const handleVideoError = () => {
			setMediaFailed(true);
		};

		try {
			art = new Artplayer({
				container: containerRef.current,
				url: videoSource,
				lang: playerLanguage,
				fullscreen: true,
				fullscreenWeb: true,
				pip: true,
				setting: true,
				playbackRate: true,
				miniProgressBar: false,
				mutex: true,
				hotkey: true,
				playsInline: true,
				airplay: true,
				moreVideoAttr: {
					preload: "metadata",
				},
			});
			videoElement = art.template.$video;
			videoElement.style.objectFit = "contain";
			videoElement.addEventListener("error", handleVideoError);
		} catch (playerError) {
			logger.warn("artplayer init failed", file.name, playerError);
			setPlayerFailed(true);
		}

		return () => {
			videoElement?.removeEventListener("error", handleVideoError);
			art?.destroy(false);
		};
	}, [file.name, mediaFailed, playerFailed, playerLanguage, videoSource]);

	if (mediaFailed) {
		return <PreviewError />;
	}

	if (playerFailed) {
		return (
			<div
				className="mx-auto w-full overflow-hidden rounded-xl bg-black"
				style={previewFrameStyle}
			>
				{/* biome-ignore lint/a11y/useMediaCaption: user-uploaded media may not have captions available */}
				<video
					src={videoSource}
					controls
					preload="metadata"
					onError={() => setMediaFailed(true)}
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
