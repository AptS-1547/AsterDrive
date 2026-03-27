import { Badge } from "@/components/ui/badge";
import type { UserRole } from "@/types/api";

export function getRoleBadgeClass(role: UserRole): string {
	return role === "admin"
		? "border-blue-500/60 bg-blue-500/10 text-blue-600 dark:text-blue-300"
		: "border-border bg-muted/40 text-muted-foreground";
}

interface RoleBadgeProps {
	role: UserRole;
	label: string;
}

export function RoleBadge({ role, label }: RoleBadgeProps) {
	return <Badge className={getRoleBadgeClass(role)}>{label}</Badge>;
}
