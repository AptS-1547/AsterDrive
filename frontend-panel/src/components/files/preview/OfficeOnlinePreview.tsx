import { useEffect, useEffectEvent, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { absoluteAppUrl } from "@/lib/publicSiteUrl";
import { cn } from "@/lib/utils";
import type { PreviewLinkInfo } from "@/types/api";
import { getFileExtension } from "./file-capabilities";
import { PreviewError } from "./PreviewError";
import { PreviewLoadingState } from "./PreviewLoadingState";
import type { PreviewableFileLike } from "./types";

const OFFICE_VIEWER_TIMEOUT_MS = 15000;

type OfficeViewerProvider = "google" | "microsoft";
type LoadPhase = "loading" | "ready" | "error";
type ErrorKind =
	| "link"
	| "timeout"
	| "publicUrlRequired"
	| "httpsRequired"
	| null;

interface OfficeOnlinePreviewProps {
	file: PreviewableFileLike;
	downloadPath: string;
	createPreviewLink: () => Promise<PreviewLinkInfo>;
}

function resolveOfficeViewerProviders(
	file: PreviewableFileLike,
): OfficeViewerProvider[] {
	const extension = getFileExtension(file);
	if (extension === "odt" || extension === "ods" || extension === "odp") {
		return ["google"];
	}
	return ["microsoft", "google"];
}

function buildOfficeViewerUrl(
	provider: OfficeViewerProvider,
	sourceUrl: string,
) {
	const encoded = encodeURIComponent(sourceUrl);
	if (provider === "google") {
		return `https://docs.google.com/gview?embedded=true&url=${encoded}`;
	}
	return `https://view.officeapps.live.com/op/embed.aspx?src=${encoded}`;
}

function isPrivateIpv4(hostname: string) {
	const parts = hostname.split(".").map((part) => Number.parseInt(part, 10));
	if (parts.length !== 4 || parts.some((part) => Number.isNaN(part))) {
		return false;
	}

	return (
		parts[0] === 10 ||
		parts[0] === 127 ||
		(parts[0] === 169 && parts[1] === 254) ||
		(parts[0] === 172 && parts[1] >= 16 && parts[1] <= 31) ||
		(parts[0] === 192 && parts[1] === 168)
	);
}

function resolveOfficeUrlError(sourceUrl: string): ErrorKind {
	let url: URL;
	try {
		url = new URL(sourceUrl);
	} catch {
		return "publicUrlRequired";
	}

	const hostname = url.hostname.toLowerCase();
	if (
		hostname === "localhost" ||
		hostname.endsWith(".localhost") ||
		hostname.endsWith(".local") ||
		hostname === "::1" ||
		hostname === "[::1]"
	) {
		return "publicUrlRequired";
	}

	if (isPrivateIpv4(hostname)) return "publicUrlRequired";

	if (hostname.includes(":")) {
		if (
			hostname.startsWith("fc") ||
			hostname.startsWith("fd") ||
			hostname.startsWith("fe80:")
		) {
			return "publicUrlRequired";
		}
	}

	const looksLikeIpv4 = /^\d{1,3}(\.\d{1,3}){3}$/.test(hostname);
	if (!looksLikeIpv4 && !hostname.includes(".")) {
		return "publicUrlRequired";
	}

	if (url.protocol !== "https:") {
		return "httpsRequired";
	}

	return null;
}

export function OfficeOnlinePreview({
	file,
	downloadPath,
	createPreviewLink,
}: OfficeOnlinePreviewProps) {
	const { t } = useTranslation("files");
	const providers = useMemo(() => resolveOfficeViewerProviders(file), [file]);
	const [provider, setProvider] = useState<OfficeViewerProvider>(
		providers[0] ?? "microsoft",
	);
	const [viewerUrl, setViewerUrl] = useState<string | null>(null);
	const [phase, setPhase] = useState<LoadPhase>("loading");
	const [errorKind, setErrorKind] = useState<ErrorKind>(null);
	const [reloadVersion, setReloadVersion] = useState(0);
	const loadVersionRef = useRef(0);
	const timeoutRef = useRef<number | null>(null);
	const loadRequestKey = `${downloadPath}:${file.mime_type}:${file.name}:${reloadVersion}`;

	useEffect(() => {
		setProvider(providers[0] ?? "microsoft");
	}, [providers]);

	const clearLoadTimeout = useEffectEvent(() => {
		if (timeoutRef.current == null) return;
		window.clearTimeout(timeoutRef.current);
		timeoutRef.current = null;
	});

	const loadViewer = useEffectEvent(
		async (
			nextProvider: OfficeViewerProvider,
			loadVersion: number,
			_requestKey: string,
		) => {
			clearLoadTimeout();
			setPhase("loading");
			setErrorKind(null);

			try {
				const previewLink = await createPreviewLink();
				if (loadVersion !== loadVersionRef.current) return;
				const sourceUrl = absoluteAppUrl(previewLink.path);
				const urlError = resolveOfficeUrlError(sourceUrl);
				if (urlError) {
					setViewerUrl(null);
					setErrorKind(urlError);
					setPhase("error");
					return;
				}
				setViewerUrl(buildOfficeViewerUrl(nextProvider, sourceUrl));
				timeoutRef.current = window.setTimeout(() => {
					if (loadVersion !== loadVersionRef.current) return;
					setErrorKind("timeout");
					setPhase("error");
				}, OFFICE_VIEWER_TIMEOUT_MS);
			} catch {
				if (loadVersion !== loadVersionRef.current) return;
				setViewerUrl(null);
				setErrorKind("link");
				setPhase("error");
			}
		},
	);

	useEffect(() => {
		const loadVersion = loadVersionRef.current + 1;
		loadVersionRef.current = loadVersion;
		void loadViewer(provider, loadVersion, loadRequestKey);
		return () => {
			clearLoadTimeout();
		};
	}, [loadRequestKey, provider]);

	const activeProviderLabel =
		provider === "google"
			? t("office_provider_google")
			: t("office_provider_microsoft");

	const handleIframeLoad = () => {
		clearLoadTimeout();
		setPhase("ready");
		setErrorKind(null);
	};

	const handleRetry = () => {
		setReloadVersion((value) => value + 1);
	};

	const openExternally = () => {
		if (!viewerUrl) return;
		window.open(viewerUrl, "_blank", "noopener,noreferrer");
	};

	const downloadFile = () => {
		window.open(downloadPath, "_blank", "noopener,noreferrer");
	};

	return (
		<div className="flex h-full min-h-[70vh] flex-col gap-3">
			<div className="flex flex-wrap items-center gap-2">
				{providers.length > 1 ? (
					<div className="inline-flex items-center rounded-lg border bg-background p-1">
						{providers.map((candidate) => {
							const isActive = candidate === provider;
							const label =
								candidate === "google"
									? t("office_provider_google")
									: t("office_provider_microsoft");
							return (
								<Button
									key={candidate}
									variant="ghost"
									size="sm"
									className={cn(
										"h-7 rounded-md px-2.5 text-xs",
										isActive && "bg-accent text-foreground",
									)}
									onClick={() => setProvider(candidate)}
								>
									<Icon name="Globe" className="mr-1 h-3.5 w-3.5" />
									{label}
								</Button>
							);
						})}
					</div>
				) : null}
				<div className="ml-auto flex flex-wrap items-center gap-2">
					{viewerUrl ? (
						<Button variant="outline" size="sm" onClick={openExternally}>
							<Icon name="ArrowSquareOut" className="mr-2 h-4 w-4" />
							{t("office_preview_open_external", {
								provider: activeProviderLabel,
							})}
						</Button>
					) : null}
					<Button variant="outline" size="sm" onClick={downloadFile}>
						<Icon name="Download" className="mr-2 h-4 w-4" />
						{t("download")}
					</Button>
				</div>
			</div>
			<div className="relative min-h-0 flex-1 overflow-hidden rounded-xl border bg-background">
				{viewerUrl ? (
					<iframe
						key={viewerUrl}
						title={file.name}
						src={viewerUrl}
						className={cn(
							"h-full w-full bg-background",
							phase !== "ready" && "pointer-events-none opacity-0",
						)}
						referrerPolicy="no-referrer"
						onLoad={handleIframeLoad}
					/>
				) : null}
				{phase === "loading" ? (
					<div className="absolute inset-0">
						<PreviewLoadingState
							text={t("office_preview_loading", {
								provider: activeProviderLabel,
							})}
							className="h-full rounded-none border-0 bg-background"
						/>
					</div>
				) : null}
				{phase === "error" ? (
					<div className="absolute inset-0 flex items-center justify-center bg-background p-6">
						<div className="flex max-w-xl flex-col items-center gap-4 text-center">
							<PreviewError onRetry={handleRetry} />
							<p className="text-sm text-muted-foreground">
								{errorKind === "timeout"
									? t("office_preview_timeout_desc", {
											provider: activeProviderLabel,
										})
									: errorKind === "publicUrlRequired"
										? t("office_preview_public_url_desc", {
												provider: activeProviderLabel,
											})
										: errorKind === "httpsRequired"
											? t("office_preview_https_required_desc", {
													provider: activeProviderLabel,
												})
											: t("office_preview_error_desc", {
													provider: activeProviderLabel,
												})}
							</p>
						</div>
					</div>
				) : null}
			</div>
		</div>
	);
}
