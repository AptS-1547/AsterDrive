import { create } from "zustand";
import {
	type AppliedBranding,
	applyBranding,
	DEFAULT_BRANDING,
	resolveBranding,
} from "@/lib/branding";
import { logger } from "@/lib/logger";
import { setPublicSiteUrls } from "@/lib/publicSiteUrl";
import { brandingService } from "@/services/brandingService";

let inFlightLoad: Promise<void> | null = null;

interface BrandingState {
	allowUserRegistration: boolean;
	branding: AppliedBranding;
	isLoaded: boolean;
	siteUrl: string | null;
	load: () => Promise<void>;
}

export const useBrandingStore = create<BrandingState>((set, get) => ({
	allowUserRegistration: true,
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
				const allowUserRegistration =
					publicBranding.allow_user_registration ?? true;
				const siteUrl = setPublicSiteUrls(publicBranding.site_urls);
				applyBranding(branding);
				set({
					allowUserRegistration,
					branding,
					isLoaded: true,
					siteUrl,
				});
			} catch (error) {
				const fallbackBranding = resolveBranding(null);
				setPublicSiteUrls(null);
				logger.warn("branding bootstrap failed, using defaults", error);
				applyBranding(fallbackBranding);
				set({
					allowUserRegistration: true,
					branding: fallbackBranding,
					isLoaded: true,
					siteUrl: null,
				});
			} finally {
				inFlightLoad = null;
			}
		})();

		return inFlightLoad;
	},
}));
