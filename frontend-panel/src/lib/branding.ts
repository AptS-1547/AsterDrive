import type { PublicBranding } from "@/types/api";

export type AppliedBranding = {
	title: string;
	description: string;
	faviconUrl: string;
};

export const DEFAULT_BRANDING: AppliedBranding = {
	title: "AsterDrive",
	description: "Self-hosted cloud storage",
	faviconUrl: "/favicon.svg",
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
		faviconUrl: normalizeFaviconUrl(branding?.favicon_url),
	};
}

export function applyBranding(branding: AppliedBranding): void {
	if (typeof document === "undefined") return;

	document.title = branding.title;
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

function normalizeFaviconUrl(value: string | null | undefined): string {
	const normalized = value?.trim();
	if (!normalized) return DEFAULT_BRANDING.faviconUrl;
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
		// invalid URLs fall back to the default favicon
	}

	return DEFAULT_BRANDING.faviconUrl;
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
