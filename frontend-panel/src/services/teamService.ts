import { api } from "@/services/http";
import type { TeamInfo } from "@/types/api";

export const teamService = {
	list: () => api.get<TeamInfo[]>("/teams"),
};
