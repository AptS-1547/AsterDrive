import { describe, expect, it } from "vitest";
import { normalizeS3ConnectionFields } from "@/lib/s3Endpoint";

describe("normalizeS3ConnectionFields", () => {
	it("trims endpoint and bucket without provider-specific rewriting", () => {
		expect(
			normalizeS3ConnectionFields(
				" https://s3.example.test/custom/path ",
				" archive ",
			),
		).toEqual({
			endpoint: "https://s3.example.test/custom/path",
			bucket: "archive",
		});
	});

	it("preserves custom endpoint paths", () => {
		expect(
			normalizeS3ConnectionFields(
				"https://s3.example.com/custom/path",
				"archive",
			),
		).toEqual({
			endpoint: "https://s3.example.com/custom/path",
			bucket: "archive",
		});
	});
});
