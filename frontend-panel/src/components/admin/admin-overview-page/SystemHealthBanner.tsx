import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import { formatDateAbsolute } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { AdminSystemHealthSummary } from "@/types/api";

function systemHealthPresentation(status: AdminSystemHealthSummary["status"]) {
	switch (status) {
		case "healthy":
			return {
				icon: "Check" as const,
				labelKey: "overview_system_health_healthy",
				className:
					"border-emerald-200 bg-emerald-50 text-emerald-950 dark:border-emerald-900 dark:bg-emerald-950/40 dark:text-emerald-100",
				iconClass:
					"bg-emerald-100 text-emerald-700 dark:bg-emerald-900 dark:text-emerald-200",
			};
		case "degraded":
			return {
				icon: "Warning" as const,
				labelKey: "overview_system_health_degraded",
				className:
					"border-amber-200 bg-amber-50 text-amber-950 dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-100",
				iconClass:
					"bg-amber-100 text-amber-700 dark:bg-amber-900 dark:text-amber-200",
			};
		case "unhealthy":
			return {
				icon: "CircleAlert" as const,
				labelKey: "overview_system_health_unhealthy",
				className:
					"border-red-200 bg-red-50 text-red-950 dark:border-red-900 dark:bg-red-950/40 dark:text-red-100",
				iconClass: "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-200",
			};
		case "unknown":
			return {
				icon: "Info" as const,
				labelKey: "overview_system_health_unknown",
				className: "border-border bg-muted/30 text-foreground dark:bg-muted/20",
				iconClass: "bg-muted text-muted-foreground",
			};
	}
}

export function SystemHealthBanner({
	health,
}: {
	health: AdminSystemHealthSummary;
}) {
	const { t } = useTranslation("admin");
	const navigate = useNavigate();
	const presentation = systemHealthPresentation(health.status);
	const checkedAt = health.checked_at
		? t("overview_system_health_checked_at", {
				date: formatDateAbsolute(health.checked_at),
			})
		: t("overview_system_health_not_checked");
	const isIssue = health.status === "degraded" || health.status === "unhealthy";
	const issueComponents = health.components.filter(
		(component) => component.status !== "healthy",
	);
	const message = isIssue
		? (health.summary ?? t("overview_system_health_no_summary"))
		: health.status === "healthy"
			? t("overview_system_health_healthy_desc")
			: t("overview_system_health_unknown_desc");

	return (
		<div
			className={cn(
				"flex flex-col gap-3 rounded-lg border px-4 py-3 md:flex-row md:items-start md:justify-between",
				presentation.className,
			)}
		>
			<div className="flex min-w-0 items-start gap-3">
				<span
					className={cn(
						"mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-full",
						presentation.iconClass,
					)}
				>
					<Icon name={presentation.icon} className="size-4" />
				</span>
				<div className="min-w-0 space-y-1">
					<div className="flex flex-wrap items-center gap-2">
						<p className="text-sm font-semibold">{t(presentation.labelKey)}</p>
						<Badge
							variant="outline"
							className="border-current/25 bg-background/40"
						>
							{checkedAt}
						</Badge>
					</div>
					<p className="break-words text-sm">{message}</p>
					{isIssue && issueComponents.length > 0 ? (
						<div className="mt-2 flex flex-wrap gap-2">
							{issueComponents.map((component) => (
								<Badge
									key={component.name}
									variant="outline"
									className="max-w-full border-current/25 bg-background/40"
									title={component.message}
								>
									<span className="truncate">
										{component.name}: {component.status}
									</span>
								</Badge>
							))}
						</div>
					) : null}
					{isIssue && issueComponents.length === 0 && health.details ? (
						<p className="break-words text-xs opacity-80">{health.details}</p>
					) : null}
				</div>
			</div>
			{health.task_id ? (
				<Button
					variant="outline"
					size="sm"
					className={cn(
						ADMIN_CONTROL_HEIGHT_CLASS,
						"shrink-0 border-current/25 bg-background/40 hover:bg-background/70",
					)}
					onClick={() => {
						void navigate("/admin/tasks?kind=system_runtime");
					}}
				>
					<Icon name="ArrowSquareOut" className="size-4" />
					{t("overview_system_health_view_history")}
				</Button>
			) : null}
		</div>
	);
}
