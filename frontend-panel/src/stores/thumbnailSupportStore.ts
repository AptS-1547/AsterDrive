import { create } from "zustand";
import { logger } from "@/lib/logger";
import { thumbnailSupportService } from "@/services/thumbnailSupportService";
import type { PublicThumbnailSupport } from "@/types/api";

let inFlightLoad: Promise<void> | null = null;
let latestLoadToken = 0;

interface ThumbnailSupportState {
	config: PublicThumbnailSupport | null;
	isLoaded: boolean;
	invalidate: () => void;
	load: (options?: { force?: boolean }) => Promise<void>;
}

export const useThumbnailSupportStore = create<ThumbnailSupportState>(
	(set, get) => ({
		config: null,
		isLoaded: false,

		invalidate: () => {
			set({
				config: null,
				isLoaded: false,
			});
		},

		load: async ({ force = false } = {}) => {
			if (!force && get().isLoaded) return;
			if (!force && inFlightLoad) return inFlightLoad;

			const loadToken = latestLoadToken + 1;
			latestLoadToken = loadToken;
			let loadPromise: Promise<void> | null = null;
			loadPromise = (async () => {
				try {
					const config = await thumbnailSupportService.get();
					if (latestLoadToken !== loadToken) return;
					set({
						config,
						isLoaded: true,
					});
				} catch (error) {
					logger.warn(
						"thumbnail support bootstrap failed, using empty support list",
						error,
					);
					if (latestLoadToken !== loadToken) return;
					set({
						config: null,
						isLoaded: true,
					});
				} finally {
					if (inFlightLoad === loadPromise) {
						inFlightLoad = null;
					}
				}
			})();

			inFlightLoad = loadPromise;

			return loadPromise;
		},
	}),
);
