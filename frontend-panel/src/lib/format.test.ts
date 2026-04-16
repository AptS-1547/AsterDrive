import { afterEach, describe, expect, it, vi } from "vitest";
import {
	formatBytes,
	formatDate,
	formatDateAbsolute,
	formatDateShort,
	formatDateTime,
	formatNumber,
} from "@/lib/format";

describe("format helpers", () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it("formats byte sizes", () => {
		expect(formatBytes(0)).toBe("0 B");
		expect(formatBytes(1536)).toBe("1.5 KB");
		expect(formatBytes(1024 * 1024)).toBe("1.0 MB");
	});

	it("formats integers with locale separators", () => {
		expect(formatNumber(0)).toBe(new Intl.NumberFormat().format(0));
		expect(formatNumber(4152537914)).toBe(
			new Intl.NumberFormat().format(4152537914),
		);
	});

	it("formats relative dates across time ranges", () => {
		vi.useFakeTimers();
		vi.setSystemTime(new Date("2026-03-28T12:00:00Z"));

		expect(formatDate("2026-03-28T11:59:40Z")).toBe("just now");
		expect(formatDate("2026-03-28T11:55:00Z")).toBe("5m ago");
		expect(formatDate("2026-03-28T10:00:00Z")).toBe("2h ago");
		expect(formatDate("2026-03-25T12:00:00Z")).toBe("3d ago");
	});

	it("delegates absolute date formatting to the built-in locale helpers", () => {
		const value = "2026-03-28T12:34:56Z";

		expect(formatDateAbsolute(value)).toBe(new Date(value).toLocaleString());
		expect(formatDateShort(value)).toBe(new Date(value).toLocaleDateString());
		expect(formatDateTime(value)).toBe(new Date(value).toLocaleString());
	});
});
