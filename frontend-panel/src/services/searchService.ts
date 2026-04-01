import { withQuery } from "@/lib/queryParams";
import { api } from "@/services/http";
import type { SearchParams, SearchResults } from "@/types/api";

export const searchService = {
	search: (params: SearchParams) =>
		api.get<SearchResults>(withQuery("/search", params)),
};
