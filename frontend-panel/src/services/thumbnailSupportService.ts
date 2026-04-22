import type { PublicThumbnailSupport } from "@/types/api";
import { api } from "./http";

export const thumbnailSupportService = {
	get: () => api.get<PublicThumbnailSupport>("/public/thumbnail-support"),
};
