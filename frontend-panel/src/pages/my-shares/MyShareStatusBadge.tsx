import { Badge } from "@/components/ui/badge";
import type { ShareStatus } from "@/types/api";

interface MyShareStatusBadgeProps {
	deletedLabel: string;
	exhaustedLabel: string;
	expiredLabel: string;
	status: ShareStatus;
	activeLabel: string;
}

export function MyShareStatusBadge({
	activeLabel,
	deletedLabel,
	exhaustedLabel,
	expiredLabel,
	status,
}: MyShareStatusBadgeProps) {
	switch (status) {
		case "active":
			return <Badge variant="secondary">{activeLabel}</Badge>;
		case "expired":
			return <Badge variant="outline">{expiredLabel}</Badge>;
		case "exhausted":
			return <Badge variant="outline">{exhaustedLabel}</Badge>;
		case "deleted":
			return <Badge variant="destructive">{deletedLabel}</Badge>;
	}
}
