import { withQuery } from "@/lib/queryParams";
import {
	PERSONAL_WORKSPACE,
	type Workspace,
	workspaceApiPrefix,
} from "@/lib/workspace";
import { api } from "@/services/http";
import { bindWorkspaceService } from "@/stores/workspaceStore";
import type { SearchParams, SearchResults } from "@/types/api";

export function createSearchService(workspace: Workspace = PERSONAL_WORKSPACE) {
	return {
		search: (params: SearchParams) =>
			api.get<SearchResults>(
				withQuery(`${workspaceApiPrefix(workspace)}/search`, params),
			),
	};
}

export const searchService = bindWorkspaceService(createSearchService);
