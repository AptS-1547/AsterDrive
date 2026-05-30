export function normalizeWebdavPrefix(prefix: string): string {
	const trimmed = prefix.trim();
	if (!trimmed) return "/webdav";
	const withLeadingSlash = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
	if (withLeadingSlash === "/") return "/";
	return withLeadingSlash.endsWith("/")
		? withLeadingSlash.slice(0, -1)
		: withLeadingSlash;
}

export function webdavEndpointPath(prefix: string): string {
	const normalized = normalizeWebdavPrefix(prefix);
	return normalized === "/" ? "/" : `${normalized}/`;
}
