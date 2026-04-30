import { describe, expect, it } from "vitest";
import { normalizePublicSiteUrls } from "@/lib/publicSiteUrl";

describe("publicSiteUrl helpers", () => {
	it("normalizes origin arrays", () => {
		expect(
			normalizePublicSiteUrls([
				" https://drive.example.com ",
				"https://panel.example.com",
				"https://drive.example.com",
			]),
		).toEqual(["https://drive.example.com", "https://panel.example.com"]);
	});

	it("rejects values that are not exact http origins", () => {
		expect(normalizePublicSiteUrls(["https://drive.example.com/app"])).toEqual(
			[],
		);
		expect(
			normalizePublicSiteUrls(["https://user:pass@drive.example.com"]),
		).toEqual([]);
		expect(normalizePublicSiteUrls(["ftp://drive.example.com"])).toEqual([]);
	});
});
