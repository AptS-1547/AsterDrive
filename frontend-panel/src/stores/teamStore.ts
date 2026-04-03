import { create } from "zustand";
import { logger } from "@/lib/logger";
import type { TeamInfo } from "@/types/api";

let inFlightEnsure: Promise<void> | null = null;

interface TeamState {
	teams: TeamInfo[];
	loading: boolean;
	loadedForUserId: number | null;
	ensureLoaded: (userId: number | null) => Promise<void>;
	clear: () => void;
}

export const useTeamStore = create<TeamState>((set, get) => ({
	teams: [],
	loading: false,
	loadedForUserId: null,
	ensureLoaded: async (userId) => {
		if (userId == null) {
			set({ teams: [], loading: false, loadedForUserId: null });
			return;
		}
		if (get().loadedForUserId === userId) return;
		if (inFlightEnsure) return inFlightEnsure;

		inFlightEnsure = (async () => {
			set({ loading: true });
			try {
				const { teamService } = await import("@/services/teamService");
				const teams = await teamService.list();
				set({
					teams,
					loading: false,
					loadedForUserId: userId,
				});
			} catch (error) {
				logger.warn("Failed to load teams", error);
				set({ teams: [], loading: false, loadedForUserId: null });
				throw error;
			} finally {
				inFlightEnsure = null;
			}
		})();

		return inFlightEnsure;
	},
	clear: () => {
		inFlightEnsure = null;
		set({ teams: [], loading: false, loadedForUserId: null });
	},
}));
