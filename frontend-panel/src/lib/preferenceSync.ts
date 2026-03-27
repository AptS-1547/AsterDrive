import { logger } from "@/lib/logger";
import { authService } from "@/services/authService";
import type { UpdatePreferencesRequest } from "@/types/api";

let pending: UpdatePreferencesRequest = {};
let timer: ReturnType<typeof setTimeout> | null = null;

export function queuePreferenceSync(patch: UpdatePreferencesRequest): void {
	Object.assign(pending, patch);
	if (timer) clearTimeout(timer);
	timer = setTimeout(async () => {
		const payload = pending;
		pending = {};
		timer = null;
		try {
			await authService.updatePreferences(payload);
		} catch (e) {
			logger.warn("preference sync failed, localStorage as fallback", e);
		}
	}, 500);
}

export function cancelPreferenceSync(): void {
	pending = {};
	if (timer) {
		clearTimeout(timer);
		timer = null;
	}
}
