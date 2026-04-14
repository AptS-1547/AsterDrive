import { beforeEach, describe, expect, it, vi } from "vitest";
import { previewAppsService } from "@/services/previewAppsService";

const apiGet = vi.hoisted(() => vi.fn());

vi.mock("@/services/http", () => ({
	api: {
		get: apiGet,
	},
}));

describe("previewAppsService", () => {
	beforeEach(() => {
		apiGet.mockReset();
	});

	it("loads public preview apps from the public endpoint", () => {
		previewAppsService.get();

		expect(apiGet).toHaveBeenCalledWith("/public/preview-apps");
	});
});
