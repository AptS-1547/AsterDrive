import { buildQueryParams, type QueryParamRecord } from "@/lib/queryParams";

export function parseOffsetSearchParam(rawValue: string | null): number {
	const parsed = Number(rawValue ?? "0");
	return Number.isNaN(parsed) ? 0 : parsed;
}

export function parsePageSizeSearchParam<PageSize extends number>(
	rawValue: string | null,
	pageSizeOptions: readonly PageSize[],
	defaultPageSize: PageSize,
): PageSize {
	const parsed = Number(rawValue ?? String(defaultPageSize));

	return pageSizeOptions.includes(parsed as PageSize)
		? (parsed as PageSize)
		: defaultPageSize;
}

export function parsePageSizeOption<PageSize extends number>(
	value: string | null,
	pageSizeOptions: readonly PageSize[],
): PageSize | null {
	if (!value) {
		return null;
	}

	const parsed = Number(value);
	return pageSizeOptions.includes(parsed as PageSize)
		? (parsed as PageSize)
		: null;
}

export function buildOffsetPaginationSearchParams<PageSize extends number>({
	offset,
	pageSize,
	defaultPageSize,
	extraParams,
}: {
	offset: number;
	pageSize: PageSize;
	defaultPageSize: PageSize;
	extraParams?: QueryParamRecord;
}): URLSearchParams {
	const query = buildQueryParams(extraParams);

	if (offset > 0) {
		query.set("offset", String(offset));
	}
	if (pageSize !== defaultPageSize) {
		query.set("pageSize", String(pageSize));
	}

	return query;
}
