export type QueryParamValue = boolean | number | string | null | undefined;

export type QueryParamRecord = Record<string, QueryParamValue>;

function isQueryParamValue(value: unknown): value is QueryParamValue {
	return (
		value == null ||
		typeof value === "boolean" ||
		typeof value === "number" ||
		typeof value === "string"
	);
}

export function buildQueryParams(params?: object): URLSearchParams {
	const query = new URLSearchParams();

	if (!params) {
		return query;
	}

	for (const [key, value] of Object.entries(params)) {
		if (!isQueryParamValue(value)) {
			continue;
		}
		if (value === undefined || value === null || value === "") {
			continue;
		}
		query.set(key, String(value));
	}

	return query;
}

export function buildQueryString(params?: object): string {
	return buildQueryParams(params).toString();
}

export function withQuery(path: string, params?: object): string {
	const query = buildQueryString(params);
	if (!query) {
		return path;
	}

	const hashIndex = path.indexOf("#");
	const hash = hashIndex >= 0 ? path.slice(hashIndex) : "";
	const base = hashIndex >= 0 ? path.slice(0, hashIndex) : path;

	if (base.endsWith("?") || base.endsWith("&")) {
		return `${base}${query}${hash}`;
	}

	if (base.includes("?")) {
		return `${base}&${query}${hash}`;
	}

	return `${base}?${query}${hash}`;
}
