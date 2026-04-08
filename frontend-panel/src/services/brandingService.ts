import type { PublicBranding } from "@/types/api";
import { api } from "./http";

export const brandingService = {
	get: () => api.get<PublicBranding>("/public/branding"),
};
