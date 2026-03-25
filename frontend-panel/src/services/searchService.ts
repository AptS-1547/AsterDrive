import { api } from "@/services/http";
import type { SearchParams, SearchResults } from "@/types/api";

export const searchService = {
	search: (params: SearchParams) => {
		const query = new URLSearchParams();
		for (const [key, value] of Object.entries(params)) {
			if (value !== undefined && value !== null && value !== "") {
				query.set(key, String(value));
			}
		}
		return api.get<SearchResults>(`/search?${query.toString()}`);
	},
};
