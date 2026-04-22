import { beforeEach, describe, expect, it, vi } from "vitest";
import { thumbnailSupportService } from "@/services/thumbnailSupportService";

const apiGet = vi.hoisted(() => vi.fn());

vi.mock("@/services/http", () => ({
	api: {
		get: apiGet,
	},
}));

describe("thumbnailSupportService", () => {
	beforeEach(() => {
		apiGet.mockReset();
	});

	it("loads public thumbnail support from the public endpoint", () => {
		thumbnailSupportService.get();

		expect(apiGet).toHaveBeenCalledWith("/public/thumbnail-support");
	});
});
