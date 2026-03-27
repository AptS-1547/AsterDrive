import { authService } from '@/services/authService';
import type { UpdatePreferencesRequest } from '@/types/api';

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
		} catch {
			// silent fail — localStorage is the fallback
		}
	}, 500);
}
