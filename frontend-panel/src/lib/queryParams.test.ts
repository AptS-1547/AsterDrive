import { describe, expect, it } from "vitest";
import {
	buildQueryParams,
	buildQueryString,
	withQuery,
} from "@/lib/queryParams";

describe("queryParams", () => {
	it("filters empty values while keeping false and zero", () => {
		expect(
			buildQueryParams({
				empty: "",
				falseValue: false,
				nullValue: null,
				offset: 0,
				undefinedValue: undefined,
			}).toString(),
		).toBe("falseValue=false&offset=0");
	});

	it("builds query strings with encoded values", () => {
		expect(
			buildQueryString({
				timezone: "Asia/Shanghai",
				keyword: "alice bob",
			}),
		).toBe("timezone=Asia%2FShanghai&keyword=alice+bob");
	});

	it("appends query strings only when parameters exist", () => {
		expect(withQuery("/admin/users")).toBe("/admin/users");
		expect(withQuery("/admin/users", { limit: 20, offset: 40 })).toBe(
			"/admin/users?limit=20&offset=40",
		);
		expect(withQuery("/admin/users?active=true")).toBe(
			"/admin/users?active=true",
		);
		expect(withQuery("/admin/users?active=true", { offset: 40 })).toBe(
			"/admin/users?active=true&offset=40",
		);
		expect(withQuery("/admin/users#section", { offset: 40 })).toBe(
			"/admin/users?offset=40#section",
		);
		expect(
			withQuery("/admin/users?active=true#section", { offset: 40 }),
		).toBe("/admin/users?active=true&offset=40#section");
	});
});
