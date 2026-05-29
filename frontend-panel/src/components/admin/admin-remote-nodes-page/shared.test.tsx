import type { TFunction } from "i18next";
import { describe, expect, it } from "vitest";
import {
	formatLastChecked,
	getRemoteNodeEnrollmentStatusLabel,
	getRemoteNodeTransportBadge,
	getRemoteNodeTransportLabel,
	getRemoteNodeTunnelLabel,
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

	it("maps transport modes to dedicated labels", () => {
		expect(getRemoteNodeTransportLabel(t, "direct")).toBe(
			"remote_node_transport_direct",
		);
		expect(getRemoteNodeTransportLabel(t, "reverse_tunnel")).toBe(
			"remote_node_transport_reverse_tunnel",
		);
		expect(getRemoteNodeTransportLabel(t, "auto")).toBe(
			"remote_node_transport_auto",
		);
	});

	it("marks reverse tunnel as a test transport", () => {
		expect(getRemoteNodeTransportBadge(t, "direct")).toBeNull();
		expect(getRemoteNodeTransportBadge(t, "reverse_tunnel")).toBe(
			"remote_node_transport_test_badge",
		);
		expect(getRemoteNodeTransportBadge(t, "auto")).toBeNull();
	});

	it("maps tunnel status from node transport state", () => {
		expect(
			getRemoteNodeTunnelLabel(t, {
				transport_mode: "direct",
				tunnel: {
					status: "online",
					last_error: "",
					last_seen_at: "2026-05-29T08:00:00Z",
				},
			} as RemoteNodeInfo),
		).toBe("remote_node_tunnel_not_used");
		expect(
			getRemoteNodeTunnelLabel(t, {
				transport_mode: "reverse_tunnel",
				tunnel: {
					status: "online",
					last_error: "",
					last_seen_at: "2026-05-29T08:00:00Z",
				},
			} as RemoteNodeInfo),
		).toBe("remote_node_tunnel_online");
		expect(
			getRemoteNodeTunnelLabel(t, {
				transport_mode: "auto",
				tunnel: {
					status: "offline",
					last_error: "poll timeout",
					last_seen_at: null,
				},
			} as RemoteNodeInfo),
		).toBe("remote_node_tunnel_offline");
	});

	it("detects completed enrollment separately from health status", () => {
		const node = {
			enrollment_status: "completed",
		} as RemoteNodeInfo;

		expect(hasCompletedRemoteNodeEnrollment(node)).toBe(true);
	});
});
