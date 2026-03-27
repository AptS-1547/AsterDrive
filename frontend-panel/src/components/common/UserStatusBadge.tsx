import { Badge } from "@/components/ui/badge";
import type { UserRole, UserStatus } from "@/types/api";

export function getRoleBadgeClass(role: UserRole): string {
	return role === "admin"
		? "border-blue-500/60 bg-blue-500/10 text-blue-600 dark:text-blue-300"
		: "border-border bg-muted/40 text-muted-foreground";
}

export function getStatusBadgeClass(status: UserStatus): string {
	return status === "active"
		? "border-green-500/60 bg-green-500/10 text-green-600 dark:text-green-300"
		: "border-amber-500/60 bg-amber-500/10 text-amber-600 dark:text-amber-300";
}

interface UserStatusBadgeProps {
	status: UserStatus;
	label: string;
}

export function UserStatusBadge({ status, label }: UserStatusBadgeProps) {
	return <Badge className={getStatusBadgeClass(status)}>{label}</Badge>;
}
