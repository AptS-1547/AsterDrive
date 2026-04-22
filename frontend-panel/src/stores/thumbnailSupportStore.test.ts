import { beforeEach, describe, expect, it, vi } from "vitest";

const mockState = vi.hoisted(() => ({
	get: vi.fn(),
	warn: vi.fn(),
}));

vi.mock("@/services/thumbnailSupportService", () => ({
	thumbnailSupportService: {
		get: (...args: unknown[]) => mockState.get(...args),
	},
}));

vi.mock("@/lib/logger", () => ({
	logger: {
		warn: (...args: unknown[]) => mockState.warn(...args),
	},
}));

const supportConfig = {
	version: 1,
	extensions: ["png", "heic"],
};

async function loadStore() {
	vi.resetModules();
	return await import("@/stores/thumbnailSupportStore");
}

describe("thumbnailSupportStore", () => {
	beforeEach(() => {
		mockState.get.mockReset();
		mockState.warn.mockReset();
	});

	it("loads public thumbnail support once and reuses the loaded state", async () => {
		mockState.get.mockResolvedValue(supportConfig);

		const { useThumbnailSupportStore } = await loadStore();

		expect(useThumbnailSupportStore.getState().config).toBeNull();
		expect(useThumbnailSupportStore.getState().isLoaded).toBe(false);

		await useThumbnailSupportStore.getState().load();

		expect(mockState.get).toHaveBeenCalledTimes(1);
		expect(useThumbnailSupportStore.getState().config).toEqual(supportConfig);
		expect(useThumbnailSupportStore.getState().isLoaded).toBe(true);

		await useThumbnailSupportStore.getState().load();

		expect(mockState.get).toHaveBeenCalledTimes(1);
	});

	it("can invalidate a failed bootstrap and force a refresh", async () => {
		mockState.get.mockRejectedValueOnce(new Error("offline"));

		const { useThumbnailSupportStore } = await loadStore();

		await useThumbnailSupportStore.getState().load();

		expect(mockState.get).toHaveBeenCalledTimes(1);
		expect(mockState.warn).toHaveBeenCalledTimes(1);
		expect(useThumbnailSupportStore.getState().config).toBeNull();
		expect(useThumbnailSupportStore.getState().isLoaded).toBe(true);

		useThumbnailSupportStore.getState().invalidate();

		expect(useThumbnailSupportStore.getState().config).toBeNull();
		expect(useThumbnailSupportStore.getState().isLoaded).toBe(false);

		mockState.get.mockResolvedValueOnce(supportConfig);

		await useThumbnailSupportStore.getState().load({ force: true });

		expect(mockState.get).toHaveBeenCalledTimes(2);
		expect(useThumbnailSupportStore.getState().config).toEqual(supportConfig);
		expect(useThumbnailSupportStore.getState().isLoaded).toBe(true);
	});
});
