import { getStatusBadgeClass } from "@/components/common/userBadgeClasses";
import { Badge } from "@/components/ui/badge";
import type { UserStatus } from "@/types/api";

interface UserStatusBadgeProps {
	status: UserStatus;
	label: string;
}

export function UserStatusBadge({ status, label }: UserStatusBadgeProps) {
	return <Badge className={getStatusBadgeClass(status)}>{label}</Badge>;
}
