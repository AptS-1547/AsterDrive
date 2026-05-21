import { describe, expect, it } from "vitest";
import {
	BUILTIN_TABLE_PREVIEW_APP_KEY,
	DEFAULT_TABLE_PREVIEW_DELIMITER,
	getTablePreviewDelimiterLabelKey,
	isTablePreviewAppKey,
	normalizeTablePreviewDelimiter,
	TABLE_PREVIEW_DELIMITER_VALUES,
} from "@/lib/tablePreview";

describe("tablePreview helpers", () => {
	it("recognizes the built-in table preview app key after trimming", () => {
		expect(isTablePreviewAppKey(BUILTIN_TABLE_PREVIEW_APP_KEY)).toBe(true);
		expect(isTablePreviewAppKey(" builtin.table ")).toBe(true);
		expect(isTablePreviewAppKey("custom.table")).toBe(false);
	});

	it("normalizes supported delimiter values and falls back for invalid input", () => {
		for (const value of TABLE_PREVIEW_DELIMITER_VALUES) {
			if (value === "\t") continue;
			expect(normalizeTablePreviewDelimiter(value)).toBe(value);
		}

		expect(normalizeTablePreviewDelimiter("\t")).toBe(
			DEFAULT_TABLE_PREVIEW_DELIMITER,
		);
		expect(normalizeTablePreviewDelimiter(" \t ")).toBe(
			DEFAULT_TABLE_PREVIEW_DELIMITER,
		);
		expect(normalizeTablePreviewDelimiter(null)).toBe(
			DEFAULT_TABLE_PREVIEW_DELIMITER,
		);
		expect(normalizeTablePreviewDelimiter("invalid")).toBe(
			DEFAULT_TABLE_PREVIEW_DELIMITER,
		);
	});

	it("maps delimiter options to translation keys", () => {
		expect(getTablePreviewDelimiterLabelKey(",")).toBe(
			"preview_apps_table_delimiter_comma",
		);
		expect(getTablePreviewDelimiterLabelKey("\t")).toBe(
			"preview_apps_table_delimiter_tab",
		);
		expect(getTablePreviewDelimiterLabelKey(";")).toBe(
			"preview_apps_table_delimiter_semicolon",
		);
		expect(getTablePreviewDelimiterLabelKey("|")).toBe(
			"preview_apps_table_delimiter_pipe",
		);
		expect(getTablePreviewDelimiterLabelKey("auto")).toBe(
			"preview_apps_table_delimiter_auto",
		);
	});
});
