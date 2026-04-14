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

	if (compact) {
		return (
			<span
				className={cn("inline-flex shrink-0 items-center gap-1", className)}
			>
				{isShared ? (
					<span
						className="flex size-5 items-center justify-center rounded-full border border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/70 dark:bg-emerald-950/40 dark:text-emerald-300"
						title={t("share")}
					>
						<span className="sr-only">{t("share")}</span>
						<Icon name="LinkSimple" className="h-3 w-3" />
					</span>
				) : null}
				{isLocked ? (
					<span
						className="flex size-5 items-center justify-center rounded-full border border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900/70 dark:bg-amber-950/40 dark:text-amber-300"
						title={t("lock")}
					>
						<span className="sr-only">{t("lock")}</span>
						<Icon name="Lock" className="h-3 w-3" />
					</span>
				) : null}
			</span>
		);
	}

	return (
		<span
			className={cn("inline-flex shrink-0 items-center gap-1.5", className)}
		>
			{isShared ? (
				<Badge
					variant="outline"
					className={cn(
						"h-5 gap-1 rounded-full px-2 text-[11px] font-medium",
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
						"h-5 gap-1 rounded-full px-2 text-[11px] font-medium",
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
