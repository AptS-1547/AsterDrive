import type { i18n as I18n } from "i18next";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	formatBytes,
	formatDate,
	formatDateAbsolute,
	formatDateShort,
	formatDateTime,
	formatNumber,
} from "@/lib/format";

function createI18nStub(
	language: "en" | "zh",
): Pick<I18n, "language" | "resolvedLanguage" | "t"> {
	return {
		language,
		resolvedLanguage: language,
		t: (key: string, options?: Record<string, unknown>) => {
			const count = Number(options?.count ?? 0);

			switch (key) {
				case "core:date_relative_just_now":
					return language === "zh" ? "刚刚" : "just now";
				case "core:date_relative_minutes_ago":
					return language === "zh" ? `${count}分钟前` : `${count}m ago`;
				case "core:date_relative_hours_ago":
					return language === "zh" ? `${count}小时前` : `${count}h ago`;
				case "core:date_relative_days_ago":
					return language === "zh" ? `${count}天前` : `${count}d ago`;
				default:
					return key;
			}
		},
	};
}

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
		const enI18n = createI18nStub("en");
		const zhI18n = createI18nStub("zh");

		expect(formatDate("2026-03-28T11:59:40Z", enI18n)).toBe("just now");
		expect(formatDate("2026-03-28T11:55:00Z", enI18n)).toBe("5m ago");
		expect(formatDate("2026-03-28T10:00:00Z", enI18n)).toBe("2h ago");
		expect(formatDate("2026-03-25T12:00:00Z", enI18n)).toBe("3d ago");
		expect(formatDate("2026-03-28T11:59:40Z", zhI18n)).toBe("刚刚");
		expect(formatDate("2026-03-28T11:55:00Z", zhI18n)).toBe("5分钟前");
		expect(formatDate("2026-03-28T10:00:00Z", zhI18n)).toBe("2小时前");
		expect(formatDate("2026-03-25T12:00:00Z", zhI18n)).toBe("3天前");
	});

	it("uses the i18n locale when formatDate falls back to calendar dates", () => {
		vi.useFakeTimers();
		vi.setSystemTime(new Date("2026-03-28T12:00:00Z"));
		const value = "2026-02-25T12:00:00Z";

		expect(formatDate(value, createI18nStub("en"))).toBe(
			new Date(value).toLocaleDateString("en"),
		);
		expect(formatDate(value, createI18nStub("zh"))).toBe(
			new Date(value).toLocaleDateString("zh"),
		);
	});

	it("delegates absolute date formatting to the built-in locale helpers", () => {
		const value = "2026-03-28T12:34:56Z";

		expect(formatDateAbsolute(value)).toBe(new Date(value).toLocaleString());
		expect(formatDateShort(value)).toBe(new Date(value).toLocaleDateString());
		expect(formatDateTime(value)).toBe(new Date(value).toLocaleString());
	});
});
