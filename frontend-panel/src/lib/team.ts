import type {
	AuditAction,
	TeamAuditEntryInfo,
	TeamMemberRole,
} from "@/types/api";
import { getUserDisplayName } from "./user";

export function isTeamManager(role: TeamMemberRole | null | undefined) {
	return role === "owner" || role === "admin";
}

export function isTeamOwner(role: TeamMemberRole | null | undefined) {
	return role === "owner";
}

export function getTeamRoleBadgeClass(role: TeamMemberRole) {
	if (role === "owner") {
		return "border-amber-500/60 bg-amber-500/10 text-amber-700 dark:text-amber-300";
	}
	if (role === "admin") {
		return "border-blue-500/60 bg-blue-500/10 text-blue-700 dark:text-blue-300";
	}
	return "border-border bg-muted/40 text-muted-foreground";
}

export function compareTeamMemberRole(a: TeamMemberRole, b: TeamMemberRole) {
	const rank: Record<TeamMemberRole, number> = {
		owner: 0,
		admin: 1,
		member: 2,
	};
	return rank[a] - rank[b];
}

export function formatTeamAuditSummary(
	entry: TeamAuditEntryInfo,
	roleLabel: (role: TeamMemberRole) => string,
) {
	if (!entry.member) {
		return null;
	}
	const memberName = getUserDisplayName(entry.member);

	if (entry.action === TEAM_MEMBER_UPDATE_ACTION) {
		if (entry.previous_role && entry.next_role) {
			return `${memberName} · ${roleLabel(entry.previous_role)} -> ${roleLabel(entry.next_role)}`;
		}

		return memberName;
	}

	if (entry.role) {
		return `${memberName} · ${roleLabel(entry.role)}`;
	}

	return memberName;
}

const TEAM_MEMBER_UPDATE_ACTION: AuditAction = "team_member_update";
