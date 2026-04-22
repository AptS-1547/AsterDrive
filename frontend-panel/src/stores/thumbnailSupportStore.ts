import { create } from "zustand";
import { logger } from "@/lib/logger";
import { thumbnailSupportService } from "@/services/thumbnailSupportService";
import type { PublicThumbnailSupport } from "@/types/api";

let inFlightLoad: Promise<void> | null = null;

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
			if (inFlightLoad) return inFlightLoad;

			inFlightLoad = (async () => {
				try {
					const config = await thumbnailSupportService.get();
					set({
						config,
						isLoaded: true,
					});
				} catch (error) {
					logger.warn(
						"thumbnail support bootstrap failed, using empty support list",
						error,
					);
					set({
						config: null,
						isLoaded: true,
					});
				} finally {
					inFlightLoad = null;
				}
			})();

			return inFlightLoad;
		},
	}),
);
