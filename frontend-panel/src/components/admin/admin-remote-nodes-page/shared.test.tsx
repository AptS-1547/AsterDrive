import type { TFunction } from "i18next";
import { describe, expect, it } from "vitest";
import { formatLastChecked } from "@/components/admin/admin-remote-nodes-page/shared";

const t = ((key: string) => key) as unknown as TFunction;

describe("admin remote nodes shared helpers", () => {
	it("formats the last checked timestamp in the browser locale and timezone", () => {
		const value = "2026-04-21T06:45:30Z";

		expect(formatLastChecked(t, value)).toBe(new Date(value).toLocaleString());
	});

	it("falls back to the never checked label when no timestamp exists", () => {
		expect(formatLastChecked(t, null)).toBe("remote_node_never_checked");
		expect(formatLastChecked(t, undefined)).toBe("remote_node_never_checked");
	});
});
