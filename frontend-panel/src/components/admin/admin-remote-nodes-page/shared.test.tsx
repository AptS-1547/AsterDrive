import type { TFunction } from "i18next";
import { describe, expect, it } from "vitest";
import {
	formatLastChecked,
	getRemoteNodeEnrollmentStatusLabel,
	hasCompletedRemoteNodeEnrollment,
} from "@/components/admin/admin-remote-nodes-page/shared";
import { getActiveDisplayTimeZone } from "@/stores/displayTimeZoneStore";
import type { RemoteNodeInfo } from "@/types/api";

const t = ((key: string) => key) as unknown as TFunction;

describe("admin remote nodes shared helpers", () => {
	it("formats the last checked timestamp in the browser locale and timezone", () => {
		const value = "2026-04-21T06:45:30Z";

		expect(formatLastChecked(t, value)).toBe(
			new Date(value).toLocaleString(undefined, {
				hour12: false,
				hourCycle: "h23",
				timeZone: getActiveDisplayTimeZone(),
			}),
		);
	});

	it("falls back to the never checked label when no timestamp exists", () => {
		expect(formatLastChecked(t, null)).toBe("remote_node_never_checked");
		expect(formatLastChecked(t, undefined)).toBe("remote_node_never_checked");
	});

	it("maps enrollment statuses to dedicated labels", () => {
		expect(getRemoteNodeEnrollmentStatusLabel(t, "not_started")).toBe(
			"remote_node_enrollment_status_not_started",
		);
		expect(getRemoteNodeEnrollmentStatusLabel(t, "pending")).toBe(
			"remote_node_enrollment_status_pending",
		);
		expect(getRemoteNodeEnrollmentStatusLabel(t, "redeemed")).toBe(
			"remote_node_enrollment_status_redeemed",
		);
		expect(getRemoteNodeEnrollmentStatusLabel(t, "completed")).toBe(
			"remote_node_enrollment_status_completed",
		);
		expect(getRemoteNodeEnrollmentStatusLabel(t, "expired")).toBe(
			"remote_node_enrollment_status_expired",
		);
	});

	it("detects completed enrollment separately from health status", () => {
		const node = {
			enrollment_status: "completed",
		} as RemoteNodeInfo;

		expect(hasCompletedRemoteNodeEnrollment(node)).toBe(true);
	});
});
