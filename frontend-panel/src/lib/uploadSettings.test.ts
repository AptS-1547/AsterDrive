import { describe, expect, it } from "vitest";
import { STORAGE_KEYS } from "@/config/app";
import {
	DEFAULT_UPLOAD_AUTO_CLEAR_COMPLETED,
	DEFAULT_UPLOAD_CONCURRENCY,
	MAX_UPLOAD_CONCURRENCY,
	MIN_UPLOAD_CONCURRENCY,
	normalizeUploadConcurrency,
	readUploadSettings,
	writeUploadAutoClearCompleted,
	writeUploadConcurrency,
} from "@/lib/uploadSettings";

describe("uploadSettings", () => {
	it("normalizes upload concurrency to supported integer bounds", () => {
		expect(normalizeUploadConcurrency(Number.NaN)).toBe(
			DEFAULT_UPLOAD_CONCURRENCY,
		);
		expect(normalizeUploadConcurrency(0)).toBe(MIN_UPLOAD_CONCURRENCY);
		expect(normalizeUploadConcurrency(4.8)).toBe(4);
		expect(normalizeUploadConcurrency(99)).toBe(MAX_UPLOAD_CONCURRENCY);
	});

	it("reads defaults when upload settings have not been stored", () => {
		expect(readUploadSettings()).toEqual({
			autoClearCompleted: DEFAULT_UPLOAD_AUTO_CLEAR_COMPLETED,
			concurrency: DEFAULT_UPLOAD_CONCURRENCY,
		});
	});

	it("persists normalized upload settings", () => {
		writeUploadConcurrency(MAX_UPLOAD_CONCURRENCY + 1);
		writeUploadAutoClearCompleted(true);

		expect(window.localStorage.getItem(STORAGE_KEYS.uploadConcurrency)).toBe(
			String(MAX_UPLOAD_CONCURRENCY),
		);
		expect(
			window.localStorage.getItem(STORAGE_KEYS.uploadAutoClearCompleted),
		).toBe("true");
		expect(readUploadSettings()).toEqual({
			autoClearCompleted: true,
			concurrency: MAX_UPLOAD_CONCURRENCY,
		});
	});
});
