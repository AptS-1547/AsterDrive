import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	computeShareExpiry,
	normalizeMaxDownloads,
	toDateTimeLocalValue,
	toIsoDateTime,
} from "@/components/files/shareDialogShared";

describe("shareDialogShared", () => {
	beforeEach(() => {
		vi.useFakeTimers();
		vi.setSystemTime(new Date("2026-04-01T08:00:00Z"));
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it("computes share expiry timestamps from preset values", () => {
		expect(computeShareExpiry("never")).toBeNull();
		expect(computeShareExpiry("1h")).toBe("2026-04-01T09:00:00.000Z");
		expect(computeShareExpiry("1d")).toBe("2026-04-02T08:00:00.000Z");
		expect(computeShareExpiry("7d")).toBe("2026-04-08T08:00:00.000Z");
		expect(computeShareExpiry("30d")).toBe("2026-05-01T08:00:00.000Z");
		expect(computeShareExpiry("unknown")).toBeNull();
	});

	it("converts ISO timestamps to datetime-local values", () => {
		const sample = new Date("2026-04-02T12:34:56Z");
		const expected = new Date(
			sample.getTime() - sample.getTimezoneOffset() * 60 * 1000,
		)
			.toISOString()
			.slice(0, 16);

		expect(toDateTimeLocalValue("2026-04-02T12:34:56Z")).toBe(expected);
		expect(toDateTimeLocalValue("invalid")).toBe("");
		expect(toDateTimeLocalValue(null)).toBe("");
	});

	it("normalizes datetime input and max downloads fields", () => {
		expect(toIsoDateTime("2026-04-03T08:30")).toBe(
			new Date("2026-04-03T08:30").toISOString(),
		);
		expect(toIsoDateTime("   ")).toBeNull();
		expect(normalizeMaxDownloads("8")).toBe(8);
		expect(normalizeMaxDownloads("-1")).toBe(0);
		expect(normalizeMaxDownloads("oops")).toBe(0);
	});
});
