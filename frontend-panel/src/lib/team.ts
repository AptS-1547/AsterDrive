import type { TeamMemberRole } from "@/types/api";

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
