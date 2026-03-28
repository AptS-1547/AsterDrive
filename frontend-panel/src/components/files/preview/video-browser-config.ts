import type { IconName } from "@/components/ui/icon";
import type { OpenWithOption, PreviewableFileLike } from "./types";

export type VideoBrowserMode = "iframe" | "new_tab";

export interface VideoBrowserConfig {
	label: string;
	mode: VideoBrowserMode;
	urlTemplate: string;
	allowedOrigins: string[];
}

export interface ResolvedVideoBrowserTarget {
	label: string;
	mode: VideoBrowserMode;
	url: string;
}

export interface VideoBrowserEnv {
	VITE_VIDEO_BROWSER_URL_TEMPLATE?: string;
	VITE_VIDEO_BROWSER_LABEL?: string;
	VITE_VIDEO_BROWSER_MODE?: string;
	VITE_VIDEO_BROWSER_ALLOWED_ORIGINS?: string;
}

export interface VideoBrowserFileContext extends PreviewableFileLike {
	id?: number;
	size?: number;
}

const DEFAULT_LABEL = "Custom Video Browser";
const TOKEN_PATTERN = /{{\s*([a-zA-Z0-9_]+)\s*}}/g;

function normalizeOrigins(value?: string) {
	return (value ?? "")
		.split(",")
		.map((item) => item.trim())
		.filter(Boolean)
		.map((origin) => {
			try {
				return new URL(origin).origin;
			} catch {
				return null;
			}
		})
		.filter((origin): origin is string => origin !== null);
}

export function parseVideoBrowserConfig(
	env: VideoBrowserEnv,
): VideoBrowserConfig | null {
	const urlTemplate = env.VITE_VIDEO_BROWSER_URL_TEMPLATE?.trim();
	if (!urlTemplate) return null;

	const label = env.VITE_VIDEO_BROWSER_LABEL?.trim() || DEFAULT_LABEL;
	const mode =
		env.VITE_VIDEO_BROWSER_MODE?.trim().toLowerCase() === "new_tab"
			? "new_tab"
			: "iframe";

	return {
		label,
		mode,
		urlTemplate,
		allowedOrigins: normalizeOrigins(env.VITE_VIDEO_BROWSER_ALLOWED_ORIGINS),
	};
}

const runtimeVideoBrowserConfig = parseVideoBrowserConfig({
	VITE_VIDEO_BROWSER_URL_TEMPLATE: import.meta.env
		.VITE_VIDEO_BROWSER_URL_TEMPLATE,
	VITE_VIDEO_BROWSER_LABEL: import.meta.env.VITE_VIDEO_BROWSER_LABEL,
	VITE_VIDEO_BROWSER_MODE: import.meta.env.VITE_VIDEO_BROWSER_MODE,
	VITE_VIDEO_BROWSER_ALLOWED_ORIGINS: import.meta.env
		.VITE_VIDEO_BROWSER_ALLOWED_ORIGINS,
});

export function getVideoBrowserConfig() {
	return runtimeVideoBrowserConfig;
}

export function getVideoBrowserOpenWithOption(
	config = runtimeVideoBrowserConfig,
): OpenWithOption | null {
	if (!config) return null;

	const icon: IconName = config.mode === "new_tab" ? "ArrowSquareOut" : "Globe";

	return {
		mode: "videoBrowser",
		labelKey: "open_with_custom_video_browser",
		label: config.label,
		icon,
	};
}

function buildTokenMap(file: VideoBrowserFileContext, downloadPath: string) {
	const origin =
		typeof window === "undefined" ? "http://localhost" : window.location.origin;
	const absoluteDownloadUrl = new URL(downloadPath, origin).toString();

	return {
		fileId: file.id != null ? String(file.id) : "",
		fileName: file.name,
		mimeType: file.mime_type,
		size: file.size != null ? String(file.size) : "",
		downloadPath,
		downloadUrl: absoluteDownloadUrl,
	};
}

function resolveTemplate(
	template: string,
	values: Record<string, string>,
): string {
	return template.replace(TOKEN_PATTERN, (_match, token: string) =>
		encodeURIComponent(values[token] ?? ""),
	);
}

export function resolveVideoBrowserTarget(
	file: VideoBrowserFileContext,
	downloadPath: string,
	config = runtimeVideoBrowserConfig,
): ResolvedVideoBrowserTarget | null {
	if (!config || typeof window === "undefined") return null;

	const resolvedUrl = resolveTemplate(
		config.urlTemplate,
		buildTokenMap(file, downloadPath),
	);

	let parsed: URL;
	try {
		parsed = new URL(resolvedUrl, window.location.origin);
	} catch {
		return null;
	}

	if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
		return null;
	}

	const isSameOrigin = parsed.origin === window.location.origin;
	const isAllowedOrigin = config.allowedOrigins.includes(parsed.origin);

	if (!isSameOrigin && !isAllowedOrigin) {
		return null;
	}

	return {
		label: config.label,
		mode: config.mode,
		url: parsed.toString(),
	};
}
