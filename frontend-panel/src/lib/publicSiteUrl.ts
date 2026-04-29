let publicSiteUrls: string[] = [];

export function normalizePublicSiteUrl(value: string | null | undefined) {
	return normalizePublicSiteUrls(value)[0] ?? null;
}

export function normalizePublicSiteUrls(value: string | null | undefined) {
	const normalized = value?.trim();
	if (!normalized) return [];

	const origins: string[] = [];
	for (const part of normalized.split(/\r\n|[,\r\n]/)) {
		const candidate = part.trim();
		if (!candidate) continue;
		try {
			const resolved = new URL(candidate);
			const validOrigin =
				(resolved.protocol === "http:" || resolved.protocol === "https:") &&
				(resolved.pathname === "" || resolved.pathname === "/") &&
				resolved.search === "" &&
				resolved.hash === "" &&
				resolved.username === "" &&
				resolved.password === "";
			if (!validOrigin) {
				return [];
			}
			if (!origins.includes(resolved.origin)) {
				origins.push(resolved.origin);
			}
		} catch {
			return [];
		}
	}

	return origins;
}

export function setPublicSiteUrl(value: string | null | undefined) {
	publicSiteUrls = normalizePublicSiteUrls(value);
	return publicSiteUrls[0] ?? null;
}

export function getPublicSiteUrl() {
	return publicSiteUrls[0] ?? null;
}

export function getPublicSiteUrls() {
	return publicSiteUrls;
}

export function publicSiteUrlMatches(value: string | null | undefined) {
	const origin = normalizePublicSiteUrl(value);
	return Boolean(origin && publicSiteUrls.includes(origin));
}

export function absoluteAppUrl(path: string) {
	if (typeof window === "undefined") return path;
	return new URL(path, getPublicSiteUrl() ?? window.location.origin).toString();
}
