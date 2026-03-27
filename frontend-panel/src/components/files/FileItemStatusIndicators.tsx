import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Icon } from "@/components/ui/icon";
import { cn } from "@/lib/utils";

interface FileItemStatusIndicatorsProps {
	isLocked?: boolean;
	isShared?: boolean;
	className?: string;
	compact?: boolean;
}

export function FileItemStatusIndicators({
	isLocked = false,
	isShared = false,
	className,
	compact = false,
}: FileItemStatusIndicatorsProps) {
	const { t } = useTranslation("files");

	if (!isLocked && !isShared) {
		return null;
	}

	const badgeClassName = compact
		? "h-5 gap-1 rounded-full px-1.5 text-[10px] font-medium"
		: "h-5 gap-1 rounded-full px-2 text-[11px] font-medium";

	return (
		<span
			className={cn("inline-flex shrink-0 items-center gap-1.5", className)}
		>
			{isShared ? (
				<Badge
					variant="outline"
					className={cn(
						badgeClassName,
						"border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/70 dark:bg-emerald-950/40 dark:text-emerald-300",
					)}
					title={t("share")}
				>
					<Icon name="LinkSimple" className="h-3 w-3" />
					<span>{t("share")}</span>
				</Badge>
			) : null}
			{isLocked ? (
				<Badge
					variant="outline"
					className={cn(
						badgeClassName,
						"border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900/70 dark:bg-amber-950/40 dark:text-amber-300",
					)}
					title={t("lock")}
				>
					<Icon name="Lock" className="h-3 w-3" />
					<span>{t("lock")}</span>
				</Badge>
			) : null}
		</span>
	);
}
