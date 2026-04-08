import { create } from "zustand";
import {
	type AppliedBranding,
	applyBranding,
	DEFAULT_BRANDING,
	resolveBranding,
} from "@/lib/branding";
import { logger } from "@/lib/logger";
import { setPublicSiteUrl } from "@/lib/publicSiteUrl";
import { brandingService } from "@/services/brandingService";

let inFlightLoad: Promise<void> | null = null;

interface BrandingState {
	branding: AppliedBranding;
	isLoaded: boolean;
	siteUrl: string | null;
	load: () => Promise<void>;
}

export const useBrandingStore = create<BrandingState>((set, get) => ({
	branding: DEFAULT_BRANDING,
	isLoaded: false,
	siteUrl: null,

	load: async () => {
		if (get().isLoaded) return;
		if (inFlightLoad) return inFlightLoad;

		inFlightLoad = (async () => {
			try {
				const publicBranding = await brandingService.get();
				const branding = resolveBranding(publicBranding);
				const siteUrl = setPublicSiteUrl(publicBranding.site_url);
				applyBranding(branding);
				set({ branding, isLoaded: true, siteUrl });
			} catch (error) {
				const fallbackBranding = resolveBranding(null);
				setPublicSiteUrl(null);
				logger.warn("branding bootstrap failed, using defaults", error);
				applyBranding(fallbackBranding);
				set({ branding: fallbackBranding, isLoaded: true, siteUrl: null });
			} finally {
				inFlightLoad = null;
			}
		})();

		return inFlightLoad;
	},
}));
