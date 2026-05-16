import { describe, expect, it } from "vitest";
import { ApiSubcode, isApiSubcode } from "@/types/api-helpers";

describe("ApiSubcode helpers", () => {
	it("accepts every runtime ApiSubcode constant", () => {
		for (const subcode of Object.values(ApiSubcode)) {
			expect(isApiSubcode(subcode)).toBe(true);
		}
	});

	it("keeps ApiSubcode runtime values unique", () => {
		const values = Object.values(ApiSubcode);

		expect(new Set(values).size).toBe(values.length);
	});

	it.each([
		"",
		"ArchivePreviewDisabled",
		"archive_preview.future_value",
		"remote.dynamic",
		"file.created",
	])("rejects non-generated or non-error subcode value %s", (value) => {
		expect(isApiSubcode(value)).toBe(false);
	});
});
