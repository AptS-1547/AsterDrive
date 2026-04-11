export const BUILTIN_TABLE_PREVIEW_APP_KEY = "builtin.table";
export const TABLE_PREVIEW_DELIMITER_VALUES = [
	"auto",
	",",
	"\t",
	";",
	"|",
] as const;

export type TablePreviewDelimiterValue =
	(typeof TABLE_PREVIEW_DELIMITER_VALUES)[number];

export const DEFAULT_TABLE_PREVIEW_DELIMITER: TablePreviewDelimiterValue =
	"auto";

export function isTablePreviewAppKey(key: string) {
	return key.trim() === BUILTIN_TABLE_PREVIEW_APP_KEY;
}

export function normalizeTablePreviewDelimiter(
	value: unknown,
): TablePreviewDelimiterValue {
	if (typeof value !== "string") {
		return DEFAULT_TABLE_PREVIEW_DELIMITER;
	}

	const normalized = value.trim();
	switch (normalized) {
		case "auto":
		case ",":
		case "\t":
		case ";":
		case "|":
			return normalized;
		default:
			return DEFAULT_TABLE_PREVIEW_DELIMITER;
	}
}

export function getTablePreviewDelimiterLabelKey(
	value: TablePreviewDelimiterValue,
) {
	switch (value) {
		case ",":
			return "preview_apps_table_delimiter_comma";
		case "\t":
			return "preview_apps_table_delimiter_tab";
		case ";":
			return "preview_apps_table_delimiter_semicolon";
		case "|":
			return "preview_apps_table_delimiter_pipe";
		default:
			return "preview_apps_table_delimiter_auto";
	}
}
