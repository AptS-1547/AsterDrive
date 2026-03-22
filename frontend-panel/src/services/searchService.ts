import { api } from "@/services/http";
import type { FileInfo, FolderInfo } from "@/types/api";

interface SearchResults {
	files: Array<FileInfo & { size: number }>;
	folders: FolderInfo[];
	total_files: number;
	total_folders: number;
}

interface SearchParams {
	q?: string;
	type?: string;
	mime_type?: string;
	min_size?: number;
	max_size?: number;
	created_after?: string;
	created_before?: string;
	folder_id?: number;
	limit?: number;
	offset?: number;
}

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
