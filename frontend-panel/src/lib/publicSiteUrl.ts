let publicSiteUrl: string | null = null;

export function normalizePublicSiteUrl(value: string | null | undefined) {
	const normalized = value?.trim();
	if (!normalized) return null;

	try {
		const resolved = new URL(normalized);
		if (resolved.protocol === "http:" || resolved.protocol === "https:") {
			return resolved.origin;
		}
	} catch {
		return null;
	}

	return null;
}

export function setPublicSiteUrl(value: string | null | undefined) {
	publicSiteUrl = normalizePublicSiteUrl(value);
	return publicSiteUrl;
}

export function getPublicSiteUrl() {
	return publicSiteUrl;
}

export function absoluteAppUrl(path: string) {
	if (typeof window === "undefined") return path;
	return new URL(path, getPublicSiteUrl() ?? window.location.origin).toString();
}
