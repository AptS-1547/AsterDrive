import type { PublicBranding } from "@/types/api";

export type AppliedBranding = {
	title: string;
	description: string;
	faviconUrl: string;
	wordmarkDarkUrl: string;
	wordmarkLightUrl: string;
};

export const DEFAULT_BRANDING: AppliedBranding = {
	title: "AsterDrive",
	description: "Self-hosted cloud storage",
	faviconUrl: "/favicon.svg",
	wordmarkDarkUrl: "/static/asterdrive/asterdrive-dark.svg",
	wordmarkLightUrl: "/static/asterdrive/asterdrive-light.svg",
};

export function resolveBranding(
	branding?: Partial<PublicBranding> | null,
): AppliedBranding {
	return {
		title: normalizeText(branding?.title, DEFAULT_BRANDING.title),
		description: normalizeText(
			branding?.description,
			DEFAULT_BRANDING.description,
		),
		faviconUrl: normalizeAssetUrl(
			branding?.favicon_url,
			DEFAULT_BRANDING.faviconUrl,
		),
		wordmarkDarkUrl: normalizeAssetUrl(
			branding?.wordmark_dark_url,
			DEFAULT_BRANDING.wordmarkDarkUrl,
		),
		wordmarkLightUrl: normalizeAssetUrl(
			branding?.wordmark_light_url,
			DEFAULT_BRANDING.wordmarkLightUrl,
		),
	};
}

export function applyBranding(branding: AppliedBranding): void {
	if (typeof document === "undefined") return;

	upsertMetaTag("description", branding.description);
	upsertLinkTag('link[rel="icon"]', {
		rel: "icon",
		href: branding.faviconUrl,
	});
	upsertLinkTag('link[rel="apple-touch-icon"]', {
		rel: "apple-touch-icon",
		href: branding.faviconUrl,
	});
}

function normalizeText(
	value: string | null | undefined,
	fallback: string,
): string {
	const normalized = value?.trim();
	return normalized ? normalized : fallback;
}

function normalizeAssetUrl(
	value: string | null | undefined,
	fallback: string,
): string {
	const normalized = value?.trim();
	if (!normalized) return fallback;
	if (
		normalized.startsWith("/") &&
		!normalized.startsWith("//") &&
		!normalized.includes(" ")
	) {
		return normalized;
	}

	try {
		const resolved = new URL(normalized);
		if (resolved.protocol === "http:" || resolved.protocol === "https:") {
			return resolved.toString();
		}
	} catch {
		// invalid URLs fall back to the default branding asset
	}

	return fallback;
}

export function formatDocumentTitle(
	appTitle: string | null | undefined,
	pageTitle?: string | null,
): string {
	const normalizedAppTitle = normalizeText(appTitle, DEFAULT_BRANDING.title);
	const normalizedPageTitle = pageTitle?.trim();

	if (!normalizedPageTitle || normalizedPageTitle === normalizedAppTitle) {
		return normalizedAppTitle;
	}

	return `${normalizedPageTitle} · ${normalizedAppTitle}`;
}

function upsertMetaTag(name: string, content: string): void {
	let meta = document.head.querySelector<HTMLMetaElement>(
		`meta[name="${name}"]`,
	);
	if (!meta) {
		meta = document.createElement("meta");
		meta.name = name;
		document.head.append(meta);
	}
	meta.content = content;
}

function upsertLinkTag(
	selector: string,
	attributes: { rel: string; href: string },
): void {
	let link = document.head.querySelector<HTMLLinkElement>(selector);
	if (!link) {
		link = document.createElement("link");
		document.head.append(link);
	}
	link.rel = attributes.rel;
	link.href = attributes.href;
	link.removeAttribute("type");
}
