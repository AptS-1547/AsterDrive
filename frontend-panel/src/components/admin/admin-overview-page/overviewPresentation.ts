import type { AdminOverview, BackgroundTaskStatus } from "@/types/api";

export const COUNT_FORMATTER = new Intl.NumberFormat();
export const DECIMAL_FORMATTER = new Intl.NumberFormat(undefined, {
	maximumFractionDigits: 1,
});

export type BackgroundTaskEvent =
	AdminOverview["recent_background_tasks"][number];

export function getOverviewBackgroundTaskStatusBadgeClass(
	status: BackgroundTaskStatus,
) {
	switch (status) {
		case "succeeded":
			return "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950/60 dark:text-emerald-300";
		case "failed":
			return "border-red-200 bg-red-50 text-red-700 dark:border-red-900 dark:bg-red-950/60 dark:text-red-300";
		case "processing":
		case "retry":
			return "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950/60 dark:text-amber-300";
		case "pending":
			return "border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-900 dark:bg-sky-950/60 dark:text-sky-300";
		case "canceled":
			return "border-border bg-muted/30 text-muted-foreground";
	}
}

export function formatOverviewRuntimeDuration(
	durationMs: number | null | undefined,
) {
	if (durationMs == null || durationMs < 0) {
		return null;
	}
	if (durationMs < 1000) {
		return `${COUNT_FORMATTER.format(durationMs)}ms`;
	}
	return `${DECIMAL_FORMATTER.format(durationMs / 1000)}s`;
}

export function backgroundTaskEventTime(task: BackgroundTaskEvent) {
	return task.finished_at ?? task.updated_at;
}
