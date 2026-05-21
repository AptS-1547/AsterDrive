import { describe, expect, it } from "vitest";
import {
	compareTeamMemberRole,
	formatTeamAuditSummary,
	getTeamRoleBadgeClass,
	isTeamManager,
	isTeamOwner,
} from "@/lib/team";
import type { TeamAuditEntryInfo, UserSummary } from "@/types/api";

const user = (overrides: Partial<UserSummary> = {}): UserSummary => ({
	id: 7,
	profile: {
		avatar: {
			source: "none",
			url_1024: null,
			url_512: null,
			version: 0,
		},
		display_name: "Ada Lovelace",
	},
	username: "ada",
	...overrides,
});

const auditEntry = (
	overrides: Partial<TeamAuditEntryInfo> = {},
): TeamAuditEntryInfo => ({
	action: "team_member_add",
	actor: user({ id: 1, username: "admin" }),
	created_at: "2026-05-01T00:00:00Z",
	id: 99,
	member: user(),
	next_role: null,
	previous_role: null,
	role: "member",
	team_id: 3,
	...overrides,
});

describe("team helpers", () => {
	it("detects manager and owner roles", () => {
		expect(isTeamManager("owner")).toBe(true);
		expect(isTeamManager("admin")).toBe(true);
		expect(isTeamManager("member")).toBe(false);
		expect(isTeamManager(null)).toBe(false);
		expect(isTeamOwner("owner")).toBe(true);
		expect(isTeamOwner("admin")).toBe(false);
	});

	it("maps roles to badge tone classes and sort order", () => {
		expect(getTeamRoleBadgeClass("owner")).toContain("amber");
		expect(getTeamRoleBadgeClass("admin")).toContain("blue");
		expect(getTeamRoleBadgeClass("member")).toContain("muted");
		expect(compareTeamMemberRole("owner", "member")).toBeLessThan(0);
		expect(compareTeamMemberRole("member", "admin")).toBeGreaterThan(0);
		expect(compareTeamMemberRole("admin", "admin")).toBe(0);
	});

	it("formats audit summaries for member changes", () => {
		const roleLabel = (role: string) => `role:${role}`;

		expect(
			formatTeamAuditSummary(
				auditEntry({
					action: "team_member_update",
					next_role: "admin",
					previous_role: "member",
					role: null,
				}),
				roleLabel,
			),
		).toBe("Ada Lovelace · role:member -> role:admin");
		expect(
			formatTeamAuditSummary(
				auditEntry({ action: "team_member_update", role: null }),
				roleLabel,
			),
		).toBe("Ada Lovelace");
		expect(formatTeamAuditSummary(auditEntry(), roleLabel)).toBe(
			"Ada Lovelace · role:member",
		);
		expect(
			formatTeamAuditSummary(
				auditEntry({ member: null, role: null }),
				roleLabel,
			),
		).toBeNull();
	});
});
