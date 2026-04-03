export function joinApiUrl(base: string, path: string) {
	const normalizedBase = base.replace(/\/+$/, "");
	const normalizedPath = path.startsWith("/") ? path : `/${path}`;
	return `${normalizedBase}${normalizedPath}`;
}
