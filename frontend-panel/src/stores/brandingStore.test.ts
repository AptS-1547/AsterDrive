import { beforeEach, describe, expect, it, vi } from "vitest";

const mockState = vi.hoisted(() => ({
	applyBranding: vi.fn(),
	getBranding: vi.fn(),
	loggerWarn: vi.fn(),
}));

vi.mock("@/services/brandingService", () => ({
	brandingService: {
		get: () => mockState.getBranding(),
	},
}));

vi.mock("@/lib/logger", () => ({
	logger: {
		warn: mockState.loggerWarn,
		error: vi.fn(),
		debug: vi.fn(),
	},
}));

vi.mock("@/lib/branding", async () => {
	const actual =
		await vi.importActual<typeof import("@/lib/branding")>("@/lib/branding");
	return {
		...actual,
		applyBranding: mockState.applyBranding,
	};
});

async function loadBrandingStoreModule() {
	vi.resetModules();
	return await import("@/stores/brandingStore");
}

describe("brandingStore", () => {
	beforeEach(() => {
		mockState.applyBranding.mockReset();
		mockState.getBranding.mockReset();
		mockState.loggerWarn.mockReset();
	});

	it("loads public branding once and applies it", async () => {
		mockState.getBranding.mockResolvedValue({
			title: "Nebula Drive",
			description: "Team storage",
			favicon_url: "https://cdn.example.com/icon.png",
		});

		const { useBrandingStore } = await loadBrandingStoreModule();

		await useBrandingStore.getState().load();
		await useBrandingStore.getState().load();

		expect(mockState.getBranding).toHaveBeenCalledTimes(1);
		expect(mockState.applyBranding).toHaveBeenCalledWith(
			expect.objectContaining({
				title: "Nebula Drive",
				description: "Team storage",
				faviconUrl: "https://cdn.example.com/icon.png",
			}),
		);
		expect(useBrandingStore.getState()).toMatchObject({
			isLoaded: true,
			branding: expect.objectContaining({
				title: "Nebula Drive",
				description: "Team storage",
			}),
		});
	});

	it("falls back to defaults when the public endpoint fails", async () => {
		mockState.getBranding.mockRejectedValue(new Error("network down"));

		const { useBrandingStore } = await loadBrandingStoreModule();

		await useBrandingStore.getState().load();

		expect(mockState.loggerWarn).toHaveBeenCalledTimes(1);
		expect(mockState.applyBranding).toHaveBeenCalledWith(
			expect.objectContaining({
				title: "AsterDrive",
				description: "Self-hosted cloud storage",
				faviconUrl: expect.stringContaining("/favicon.svg"),
			}),
		);
		expect(useBrandingStore.getState()).toMatchObject({
			isLoaded: true,
			branding: expect.objectContaining({
				title: "AsterDrive",
				description: "Self-hosted cloud storage",
			}),
		});
	});
});
