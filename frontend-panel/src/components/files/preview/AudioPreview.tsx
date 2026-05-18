import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { resolveApiResourceUrl } from "@/lib/apiUrl";
import { logger } from "@/lib/logger";
import type { ShareStreamSessionInfo } from "@/types/api";
import { PreviewError } from "./PreviewError";
import { PreviewLoadingState } from "./PreviewLoadingState";
import type { PreviewableFileLike } from "./types";

interface AudioPreviewProps {
	file: PreviewableFileLike;
	mediaStreamLinkFactory?: () => Promise<ShareStreamSessionInfo>;
	path: string;
}

export function AudioPreview({
	file,
	mediaStreamLinkFactory,
	path,
}: AudioPreviewProps) {
	const { t } = useTranslation("files");
	const [resolvedPath, setResolvedPath] = useState<string | null>(
		mediaStreamLinkFactory ? null : path,
	);
	const [streamLinkFailed, setStreamLinkFailed] = useState(false);
	const [mediaFailed, setMediaFailed] = useState(false);
	const audioSource = useMemo(
		() => (resolvedPath ? resolveApiResourceUrl(resolvedPath) : null),
		[resolvedPath],
	);

	useEffect(() => {
		let cancelled = false;
		setStreamLinkFailed(false);
		setMediaFailed(false);

		if (!mediaStreamLinkFactory) {
			setResolvedPath(path);
			return () => {
				cancelled = true;
			};
		}

		setResolvedPath(null);
		mediaStreamLinkFactory()
			.then((link) => {
				if (cancelled) return;
				setResolvedPath(link.path);
			})
			.catch((error) => {
				if (cancelled) return;
				logger.warn("audio stream session creation failed", file.name, error);
				setStreamLinkFailed(true);
			});

		return () => {
			cancelled = true;
		};
	}, [file.name, path, mediaStreamLinkFactory]);

	if (streamLinkFailed || mediaFailed) {
		return <PreviewError />;
	}

	if (!audioSource) {
		return <PreviewLoadingState text={t("loading_preview")} />;
	}

	return (
		<div className="flex min-h-[50vh] items-center justify-center px-6">
			{/* biome-ignore lint/a11y/useMediaCaption: user-uploaded media may not have captions available */}
			<audio
				src={audioSource}
				controls
				preload="metadata"
				onError={() => setMediaFailed(true)}
				className="w-full max-w-3xl"
			/>
		</div>
	);
}
