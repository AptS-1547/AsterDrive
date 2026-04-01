import { describe, expect, it } from "vitest";
import {
	buildOffsetPaginationSearchParams,
	parseOffsetSearchParam,
	parsePageSizeOption,
	parsePageSizeSearchParam,
} from "@/lib/pagination";

describe("pagination helpers", () => {
	it("parses offset values and falls back to zero", () => {
		expect(parseOffsetSearchParam("24")).toBe(24);
		expect(parseOffsetSearchParam("invalid")).toBe(0);
		expect(parseOffsetSearchParam("-1")).toBe(0);
		expect(parseOffsetSearchParam("1.5")).toBe(0);
		expect(parseOffsetSearchParam("Infinity")).toBe(0);
	});

	it("accepts only supported page sizes", () => {
		expect(parsePageSizeSearchParam("50", [10, 20, 50] as const, 20)).toBe(50);
		expect(parsePageSizeSearchParam("99", [10, 20, 50] as const, 20)).toBe(20);
		expect(parsePageSizeOption("10", [10, 20, 50] as const)).toBe(10);
		expect(parsePageSizeOption("11", [10, 20, 50] as const)).toBeNull();
	});

	it("builds pagination search params while omitting defaults", () => {
		expect(
			buildOffsetPaginationSearchParams({
				offset: 0,
				pageSize: 20,
				defaultPageSize: 20,
				extraParams: {
					keyword: "alice",
					offset: 999,
					pageSize: 999,
					role: "__all__",
					status: undefined,
				},
			}).toString(),
		).toBe("keyword=alice&role=__all__");

		expect(
			buildOffsetPaginationSearchParams({
				offset: 40,
				pageSize: 50,
				defaultPageSize: 20,
			}).toString(),
		).toBe("offset=40&pageSize=50");
	});
});
