import { isExternalResourceUrl, isPublicResourcePath } from "@/lib/apiUrl";
import { isSessionAuthFailure } from "@/lib/authErrors";
import { logger } from "@/lib/logger";
import { api } from "@/services/http";
import { useAuthStore } from "@/stores/authStore";

function shouldProbeAuthenticatedResource(path: string) {
	return !isExternalResourceUrl(path) && !isPublicResourcePath(path);
}

export async function prepareAuthenticatedResource(
	path: string,
): Promise<void> {
	if (!shouldProbeAuthenticatedResource(path)) return;

	await useAuthStore.getState().ensureFreshSession();

	try {
		await api.client.get(path, {
			headers: {
				Range: "bytes=0-0",
			},
			responseType: "blob",
			validateStatus: (status) =>
				(status >= 200 && status < 400) || status === 416,
		});
	} catch (error) {
		if (isSessionAuthFailure(error)) {
			throw error;
		}
		logger.debug("authenticated resource probe failed", path, error);
	}
}
