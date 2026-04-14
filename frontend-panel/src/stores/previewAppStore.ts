import { create } from "zustand";
import { logger } from "@/lib/logger";
import { previewAppsService } from "@/services/previewAppsService";
import type { PublicPreviewAppsConfig } from "@/types/api";

export const PREVIEW_APPS_CACHE_KEY = "aster-cached-preview-apps";

interface CachedPreviewAppsPayload {
	config: PublicPreviewAppsConfig;
}

function readCachedPreviewApps(): PublicPreviewAppsConfig | null {
	try {
		const raw = localStorage.getItem(PREVIEW_APPS_CACHE_KEY);
		if (!raw) {
			return null;
		}

		const parsed = JSON.parse(raw) as CachedPreviewAppsPayload | null;
		if (!parsed || typeof parsed !== "object" || !("config" in parsed)) {
			localStorage.removeItem(PREVIEW_APPS_CACHE_KEY);
			return null;
		}

		return parsed.config;
	} catch {
		try {
			localStorage.removeItem(PREVIEW_APPS_CACHE_KEY);
		} catch {
			// ignore storage failures
		}
		return null;
	}
}

function writeCachedPreviewApps(config: PublicPreviewAppsConfig) {
	try {
		localStorage.setItem(
			PREVIEW_APPS_CACHE_KEY,
			JSON.stringify({ config } satisfies CachedPreviewAppsPayload),
		);
	} catch {
		// ignore storage failures
	}
}

function clearCachedPreviewApps() {
	try {
		localStorage.removeItem(PREVIEW_APPS_CACHE_KEY);
	} catch {
		// ignore storage failures
	}
}

const initialCachedConfig = readCachedPreviewApps();
let inFlightLoad: Promise<void> | null = null;
let hasRevalidatedThisSession = false;

interface PreviewAppState {
	config: PublicPreviewAppsConfig | null;
	isLoaded: boolean;
	invalidate: () => void;
	load: (options?: { force?: boolean }) => Promise<void>;
}

export const usePreviewAppStore = create<PreviewAppState>((set) => ({
	config: initialCachedConfig,
	isLoaded: initialCachedConfig !== null,

	invalidate: () => {
		clearCachedPreviewApps();
		hasRevalidatedThisSession = false;
		set({
			config: null,
			isLoaded: false,
		});
	},

	load: async ({ force = false } = {}) => {
		if (!force && hasRevalidatedThisSession) return;
		if (inFlightLoad) return inFlightLoad;

		inFlightLoad = (async () => {
			try {
				const config = await previewAppsService.get();
				writeCachedPreviewApps(config);
				set({
					config,
					isLoaded: true,
				});
			} catch (error) {
				logger.warn(
					"preview apps bootstrap failed, using local fallback",
					error,
				);
				set((state) =>
					state.isLoaded
						? state
						: {
								config: null,
								isLoaded: true,
							},
				);
			} finally {
				hasRevalidatedThisSession = true;
				inFlightLoad = null;
			}
		})();

		return inFlightLoad;
	},
}));
