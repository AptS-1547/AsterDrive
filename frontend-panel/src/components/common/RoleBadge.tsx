import { getRoleBadgeClass } from "@/components/common/userBadgeClasses";
import { Badge } from "@/components/ui/badge";
import type { UserRole } from "@/types/api";

interface RoleBadgeProps {
	role: UserRole;
	label: string;
}

export function RoleBadge({ role, label }: RoleBadgeProps) {
	return <Badge className={getRoleBadgeClass(role)}>{label}</Badge>;
}
