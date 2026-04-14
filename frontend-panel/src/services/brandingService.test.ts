import { beforeEach, describe, expect, it, vi } from "vitest";
import { brandingService } from "@/services/brandingService";

const apiGet = vi.hoisted(() => vi.fn());

vi.mock("@/services/http", () => ({
	api: {
		get: apiGet,
	},
}));

describe("brandingService", () => {
	beforeEach(() => {
		apiGet.mockReset();
	});

	it("loads public branding from the public endpoint", () => {
		brandingService.get();

		expect(apiGet).toHaveBeenCalledWith("/public/branding");
	});
});
