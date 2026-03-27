import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";

export type StatusType = "active" | "expired" | "disabled";

interface StatusBadgeProps {
	status: StatusType;
}

export function StatusBadge({ status }: StatusBadgeProps) {
	const { t } = useTranslation("common");

	const classMap: Record<StatusType, string> = {
		active:
			"bg-emerald-500/10 text-emerald-700 dark:text-emerald-300 border-emerald-500/30",
		expired: "bg-red-500/10 text-red-700 dark:text-red-300 border-red-500/30",
		disabled:
			"bg-gray-500/10 text-gray-700 dark:text-gray-300 border-gray-500/30",
	};

	return (
		<Badge variant="outline" className={classMap[status]}>
			{t(status)}
		</Badge>
	);
}
