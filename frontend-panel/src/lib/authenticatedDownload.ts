import { resolveApiResourceUrl } from "@/lib/apiUrl";
import { useAuthStore } from "@/stores/authStore";

function triggerBrowserDownload(url: string) {
	const anchor = document.createElement("a");
	anchor.href = resolveApiResourceUrl(url);
	anchor.download = "";
	anchor.click();
}

export async function startAuthenticatedDownload(path: string): Promise<void> {
	await useAuthStore.getState().ensureFreshSession();
	triggerBrowserDownload(path);
}
