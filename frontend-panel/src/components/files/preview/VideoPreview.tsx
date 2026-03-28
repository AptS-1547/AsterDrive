import Artplayer from "artplayer";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useBlobUrl } from "@/hooks/useBlobUrl";
import { logger } from "@/lib/logger";
import { PreviewError } from "./PreviewError";
import type { PreviewableFileLike } from "./types";

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

	const playerLanguage = useMemo(
		() => getPlayerLanguage(i18n.language),
		[i18n.language],
	);

	useEffect(() => {
		if (!blobUrl || !containerRef.current || playerFailed) return;

		let art: Artplayer | null = null;

		try {
			art = new Artplayer({
				container: containerRef.current,
				url: blobUrl,
				lang: playerLanguage,
				autoSize: true,
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
			<div className="p-6 text-sm text-muted-foreground">
				{t("loading_preview")}
			</div>
		);
	}

	if (error || !blobUrl) {
		return <PreviewError onRetry={retry} />;
	}

	if (playerFailed) {
		return (
			<div className="mx-auto flex min-h-[60vh] w-full max-w-5xl items-center justify-center">
				{/* biome-ignore lint/a11y/useMediaCaption: user-uploaded media may not have captions available */}
				<video
					src={blobUrl}
					controls
					className="max-h-full max-w-full rounded-xl"
				/>
			</div>
		);
	}

	return (
		<div className="mx-auto h-full min-h-[60vh] w-full max-w-5xl overflow-hidden rounded-xl bg-black">
			<div ref={containerRef} className="h-full w-full" />
		</div>
	);
}
