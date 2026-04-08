import { create } from "zustand";
import {
	type AppliedBranding,
	applyBranding,
	DEFAULT_BRANDING,
	resolveBranding,
} from "@/lib/branding";
import { logger } from "@/lib/logger";
import { brandingService } from "@/services/brandingService";

let inFlightLoad: Promise<void> | null = null;

interface BrandingState {
	branding: AppliedBranding;
	isLoaded: boolean;
	load: () => Promise<void>;
}

export const useBrandingStore = create<BrandingState>((set, get) => ({
	branding: DEFAULT_BRANDING,
	isLoaded: false,

	load: async () => {
		if (get().isLoaded) return;
		if (inFlightLoad) return inFlightLoad;

		inFlightLoad = (async () => {
			try {
				const branding = resolveBranding(await brandingService.get());
				applyBranding(branding);
				set({ branding, isLoaded: true });
			} catch (error) {
				const fallbackBranding = resolveBranding(null);
				logger.warn("branding bootstrap failed, using defaults", error);
				applyBranding(fallbackBranding);
				set({ branding: fallbackBranding, isLoaded: true });
			} finally {
				inFlightLoad = null;
			}
		})();

		return inFlightLoad;
	},
}));
