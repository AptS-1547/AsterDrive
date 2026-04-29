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
	beforeEach(async () => {
		mockState.applyBranding.mockReset();
		mockState.getBranding.mockReset();
		mockState.loggerWarn.mockReset();
		const { setPublicSiteUrl } = await import("@/lib/publicSiteUrl");
		setPublicSiteUrl(null);
	});

	it("loads public branding once and applies it", async () => {
		mockState.getBranding.mockResolvedValue({
			allow_user_registration: false,
			title: "Nebula Drive",
			description: "Team storage",
			favicon_url: "https://cdn.example.com/icon.png",
			wordmark_dark_url: "https://cdn.example.com/wordmark-dark.svg",
			wordmark_light_url: "https://cdn.example.com/wordmark-light.svg",
			site_url: "https://drive.example.com",
			site_url_raw: "https://drive.example.com\nhttps://panel.example.com",
		});

		const { useBrandingStore } = await loadBrandingStoreModule();
		const { getPublicSiteUrl, getPublicSiteUrls } = await import(
			"@/lib/publicSiteUrl"
		);

		await useBrandingStore.getState().load();
		await useBrandingStore.getState().load();

		expect(mockState.getBranding).toHaveBeenCalledTimes(1);
		expect(mockState.applyBranding).toHaveBeenCalledWith(
			expect.objectContaining({
				title: "Nebula Drive",
				description: "Team storage",
				faviconUrl: "https://cdn.example.com/icon.png",
				wordmarkDarkUrl: "https://cdn.example.com/wordmark-dark.svg",
				wordmarkLightUrl: "https://cdn.example.com/wordmark-light.svg",
			}),
		);
		expect(useBrandingStore.getState()).toMatchObject({
			allowUserRegistration: false,
			isLoaded: true,
			branding: expect.objectContaining({
				title: "Nebula Drive",
				description: "Team storage",
			}),
			siteUrl: "https://drive.example.com",
		});
		expect(getPublicSiteUrl()).toBe("https://drive.example.com");
		expect(getPublicSiteUrls()).toEqual([
			"https://drive.example.com",
			"https://panel.example.com",
		]);
	});

	it("falls back to defaults when the public endpoint fails", async () => {
		mockState.getBranding.mockRejectedValue(new Error("network down"));

		const { useBrandingStore } = await loadBrandingStoreModule();
		const { getPublicSiteUrl } = await import("@/lib/publicSiteUrl");

		await useBrandingStore.getState().load();

		expect(mockState.loggerWarn).toHaveBeenCalledTimes(1);
		expect(mockState.applyBranding).toHaveBeenCalledWith(
			expect.objectContaining({
				title: "AsterDrive",
				description: "Self-hosted cloud storage",
				faviconUrl: expect.stringContaining("/favicon.svg"),
				wordmarkDarkUrl: expect.stringContaining(
					"/static/asterdrive/asterdrive-dark.svg",
				),
				wordmarkLightUrl: expect.stringContaining(
					"/static/asterdrive/asterdrive-light.svg",
				),
			}),
		);
		expect(useBrandingStore.getState()).toMatchObject({
			allowUserRegistration: true,
			isLoaded: true,
			branding: expect.objectContaining({
				title: "AsterDrive",
				description: "Self-hosted cloud storage",
			}),
			siteUrl: null,
		});
		expect(getPublicSiteUrl()).toBeNull();
	});
});
