export type QueryParamValue = boolean | number | string | null | undefined;

export type QueryParamRecord = Record<string, QueryParamValue>;

export function buildQueryParams(params?: QueryParamRecord): URLSearchParams {
	const query = new URLSearchParams();

	if (!params) {
		return query;
	}

	for (const [key, value] of Object.entries(params)) {
		if (value === undefined || value === null || value === "") {
			continue;
		}
		query.set(key, String(value));
	}

	return query;
}

export function buildQueryString(params?: QueryParamRecord): string {
	return buildQueryParams(params).toString();
}

export function withQuery(path: string, params?: QueryParamRecord): string {
	const query = buildQueryString(params);
	return query ? `${path}?${query}` : path;
}
