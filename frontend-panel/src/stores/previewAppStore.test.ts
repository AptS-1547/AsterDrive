import { beforeEach, describe, expect, it, vi } from "vitest";

const mockState = vi.hoisted(() => ({
	get: vi.fn(),
	warn: vi.fn(),
}));

vi.mock("@/services/previewAppsService", () => ({
	previewAppsService: {
		get: (...args: unknown[]) => mockState.get(...args),
	},
}));

vi.mock("@/lib/logger", () => ({
	logger: {
		warn: (...args: unknown[]) => mockState.warn(...args),
	},
}));

const cachedConfig = {
	version: 2,
	apps: [
		{
			extensions: ["md"],
			icon: "Scroll",
			key: "builtin.markdown",
			label_i18n_key: "open_with_markdown",
			provider: "builtin",
		},
	],
};

const freshConfig = {
	version: 2,
	apps: [
		{
			extensions: ["md"],
			icon: "FileCode",
			key: "builtin.code",
			label_i18n_key: "open_with_code",
			provider: "builtin",
		},
	],
};

async function loadStore() {
	vi.resetModules();
	return await import("@/stores/previewAppStore");
}

describe("previewAppStore", () => {
	beforeEach(() => {
		localStorage.clear();
		mockState.get.mockReset();
		mockState.warn.mockReset();
	});

	it("hydrates cached config immediately and revalidates it once per session", async () => {
		localStorage.setItem(
			"aster-cached-preview-apps",
			JSON.stringify({ config: cachedConfig }),
		);
		mockState.get.mockResolvedValue(freshConfig);

		const { PREVIEW_APPS_CACHE_KEY, usePreviewAppStore } = await loadStore();

		expect(usePreviewAppStore.getState().config).toEqual(cachedConfig);
		expect(usePreviewAppStore.getState().isLoaded).toBe(true);

		await usePreviewAppStore.getState().load();

		expect(mockState.get).toHaveBeenCalledTimes(1);
		expect(usePreviewAppStore.getState().config).toEqual(freshConfig);
		expect(
			JSON.parse(localStorage.getItem(PREVIEW_APPS_CACHE_KEY) ?? "null"),
		).toEqual({
			config: freshConfig,
		});

		await usePreviewAppStore.getState().load();

		expect(mockState.get).toHaveBeenCalledTimes(1);
	});

	it("keeps cached config on failed revalidation and can invalidate before a forced refresh", async () => {
		localStorage.setItem(
			"aster-cached-preview-apps",
			JSON.stringify({ config: cachedConfig }),
		);
		mockState.get.mockRejectedValueOnce(new Error("offline"));

		const { PREVIEW_APPS_CACHE_KEY, usePreviewAppStore } = await loadStore();

		await usePreviewAppStore.getState().load();

		expect(usePreviewAppStore.getState().config).toEqual(cachedConfig);
		expect(usePreviewAppStore.getState().isLoaded).toBe(true);
		expect(mockState.warn).toHaveBeenCalledTimes(1);

		usePreviewAppStore.getState().invalidate();

		expect(localStorage.getItem(PREVIEW_APPS_CACHE_KEY)).toBeNull();
		expect(usePreviewAppStore.getState().config).toBeNull();
		expect(usePreviewAppStore.getState().isLoaded).toBe(false);

		mockState.get.mockResolvedValueOnce(freshConfig);

		await usePreviewAppStore.getState().load({ force: true });

		expect(usePreviewAppStore.getState().config).toEqual(freshConfig);
		expect(usePreviewAppStore.getState().isLoaded).toBe(true);
		expect(
			JSON.parse(localStorage.getItem(PREVIEW_APPS_CACHE_KEY) ?? "null"),
		).toEqual({
			config: freshConfig,
		});
	});
});
