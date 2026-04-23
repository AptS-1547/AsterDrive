import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
	CartesianGrid,
	Line,
	LineChart,
	ResponsiveContainer,
	Tooltip,
	type TooltipContentProps,
	XAxis,
	YAxis,
} from "recharts";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { AdminPageHeader } from "@/components/layout/AdminPageHeader";
import { AdminPageShell } from "@/components/layout/AdminPageShell";
import { AdminSurface } from "@/components/layout/AdminSurface";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription } from "@/components/ui/card";
import { Icon, type IconName } from "@/components/ui/icon";
import { Skeleton } from "@/components/ui/skeleton";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { usePageTitle } from "@/hooks/usePageTitle";
import {
	ADMIN_CONTROL_HEIGHT_CLASS,
	PAGE_SECTION_PADDING_CLASS,
} from "@/lib/constants";
import {
	formatBytes,
	formatDateAbsolute,
	formatDateAbsoluteWithOffset,
} from "@/lib/format";
import { cn } from "@/lib/utils";
import { adminOverviewService } from "@/services/adminService";
import {
	resolveActiveDisplayTimeZone,
	useDisplayTimeZoneStore,
} from "@/stores/displayTimeZoneStore";
import type {
	AdminOverview,
	AuditAction,
	BackgroundTaskKind,
	BackgroundTaskStatus,
} from "@/types/api";

const COUNT_FORMATTER = new Intl.NumberFormat();
const DECIMAL_FORMATTER = new Intl.NumberFormat(undefined, {
	maximumFractionDigits: 1,
});
const OVERVIEW_TREND_DAYS = 7;
const DEFAULT_EVENT_LIMIT = 10;

type DailyReport = AdminOverview["daily_reports"][number];
type BackgroundTaskEvent = AdminOverview["recent_background_tasks"][number];
type TrendSeriesKey = "newUsers" | "shareCreations" | "uploads";

interface TrendPoint {
	date: string;
	label: string;
	newUsers: number;
	shareCreations: number;
	uploads: number;
}

interface TrendSeries {
	badgeClass: string;
	key: TrendSeriesKey;
	label: string;
	stroke: string;
	strokeWidth: number;
}

interface StatCardProps {
	label: string;
	value: string;
	icon: IconName;
	accentClass: string;
}

function StatCard({ label, value, icon, accentClass }: StatCardProps) {
	return (
		<Card className="border-0 shadow-none ring-1 ring-border/70">
			<CardContent className="flex items-start justify-between gap-3 p-4">
				<div className="min-w-0 space-y-1">
					<CardDescription className="text-xs leading-5">
						{label}
					</CardDescription>
					<p className="text-2xl font-semibold tracking-tight">{value}</p>
				</div>
				<div
					className={cn(
						"mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl",
						accentClass,
					)}
				>
					<Icon name={icon} className="h-4 w-4" />
				</div>
			</CardContent>
		</Card>
	);
}

function StatCardSkeleton() {
	return (
		<Card className="border-0 shadow-none ring-1 ring-border/70">
			<CardContent className="flex items-start justify-between gap-3 p-4">
				<div className="space-y-2">
					<Skeleton className="h-3.5 w-24" />
					<Skeleton className="h-7 w-20" />
				</div>
				<Skeleton className="h-9 w-9 rounded-xl" />
			</CardContent>
		</Card>
	);
}

function getActionBadgeClass(action: AuditAction) {
	if (action.includes("delete")) {
		return "border-red-200 bg-red-50 text-red-700 dark:border-red-900 dark:bg-red-950/60 dark:text-red-300";
	}
	if (action.includes("upload")) {
		return "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950/60 dark:text-emerald-300";
	}
	if (action.includes("share")) {
		return "border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-900 dark:bg-sky-950/60 dark:text-sky-300";
	}
	if (action.includes("login")) {
		return "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950/60 dark:text-amber-300";
	}
	return "border-border bg-muted/30 text-muted-foreground";
}

function getBackgroundTaskStatusBadgeClass(status: BackgroundTaskStatus) {
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

function formatRuntimeDuration(durationMs: number | null | undefined) {
	if (durationMs == null || durationMs < 0) {
		return null;
	}
	if (durationMs < 1000) {
		return `${COUNT_FORMATTER.format(durationMs)}ms`;
	}
	return `${DECIMAL_FORMATTER.format(durationMs / 1000)}s`;
}

function backgroundTaskEventTime(task: BackgroundTaskEvent) {
	return task.finished_at ?? task.updated_at;
}

function formatTrendDayLabel(date: string) {
	const [year, month, day] = date.split("-");
	if (!year || !month || !day) return date;
	return `${Number(month)}/${Number(day)}`;
}

function sortReportsByDateAscending(reports: DailyReport[]) {
	return [...reports].sort((left, right) =>
		left.date.localeCompare(right.date),
	);
}

function createTrendData(reports: DailyReport[]): TrendPoint[] {
	return reports.map((report) => ({
		date: report.date,
		label: formatTrendDayLabel(report.date),
		newUsers: report.new_users,
		shareCreations: report.share_creations,
		uploads: report.uploads,
	}));
}

function resolveTooltipValue(rawValue: unknown) {
	const numericValue = Array.isArray(rawValue)
		? Number(rawValue[0] ?? 0)
		: Number(rawValue ?? 0);

	return Number.isFinite(numericValue) ? numericValue : 0;
}

interface TrendTooltipCardProps extends TooltipContentProps {
	series: TrendSeries[];
}

function TrendTooltipCard({ active, payload, series }: TrendTooltipCardProps) {
	if (!active || !payload?.length) return null;

	const point = payload[0]?.payload as TrendPoint | undefined;

	return (
		<div className="rounded-xl border bg-background/95 px-3 py-2 shadow-lg backdrop-blur">
			<p className="text-xs text-muted-foreground">{point?.date ?? "---"}</p>
			<div className="mt-2 space-y-1.5">
				{series.map((seriesItem) => {
					const currentPayload = payload.find(
						(entry) => entry.dataKey === seriesItem.key,
					);

					return (
						<div
							key={seriesItem.key}
							className="flex items-center justify-between gap-4 text-xs"
						>
							<div className="flex items-center gap-2 text-muted-foreground">
								<span
									className="inline-flex size-2 rounded-full"
									style={{ backgroundColor: seriesItem.stroke }}
								/>
								<span>{seriesItem.label}</span>
							</div>
							<span className="font-semibold text-foreground">
								{COUNT_FORMATTER.format(
									resolveTooltipValue(currentPayload?.value),
								)}
							</span>
						</div>
					);
				})}
			</div>
		</div>
	);
}

interface OverviewTrendChartProps {
	reports: DailyReport[];
	emptyTitle: string;
	emptyDescription: string;
	averageLabel: string;
	latestLabel: string;
	peakLabel: string;
	series: TrendSeries[];
}

function OverviewTrendChart({
	reports,
	emptyTitle,
	emptyDescription,
	averageLabel,
	latestLabel,
	peakLabel,
	series,
}: OverviewTrendChartProps) {
	if (!reports.length) {
		return (
			<EmptyState
				icon={<Icon name="Presentation" className="h-10 w-10" />}
				title={emptyTitle}
				description={emptyDescription}
			/>
		);
	}

	const orderedReports = sortReportsByDateAscending(reports);
	const trendData = createTrendData(orderedReports);
	const latestReport = orderedReports[orderedReports.length - 1];
	const totalEvents = orderedReports.reduce(
		(sum, report) => sum + report.total_events,
		0,
	);
	const averageEvents = totalEvents / orderedReports.length;
	const peakReport = orderedReports.reduce((peak, report) =>
		report.total_events > peak.total_events ? report : peak,
	);

	return (
		<div className="grid min-w-0 gap-4 xl:grid-cols-[minmax(0,1fr)_220px]">
			<div className="min-w-0 overflow-hidden rounded-2xl border bg-linear-to-br from-primary/5 via-background to-background p-4">
				<div className="mb-3 flex flex-wrap items-center gap-2">
					{series.map((seriesItem) => (
						<Badge
							key={seriesItem.key}
							variant="outline"
							className={cn("gap-2 border", seriesItem.badgeClass)}
						>
							<span
								className="inline-flex size-2 rounded-full"
								style={{ backgroundColor: seriesItem.stroke }}
							/>
							{seriesItem.label}
						</Badge>
					))}
				</div>
				<div className="h-[280px] min-w-0 min-h-[280px]">
					<ResponsiveContainer width="100%" height="100%">
						<LineChart
							data={trendData}
							margin={{ top: 8, right: 8, left: -24, bottom: 0 }}
						>
							<CartesianGrid
								vertical={false}
								stroke="var(--border)"
								strokeDasharray="4 6"
							/>
							<XAxis
								dataKey="label"
								axisLine={false}
								tickLine={false}
								tickMargin={12}
								interval={0}
								minTickGap={0}
								padding={{ left: 12, right: 12 }}
								tick={{ fill: "var(--muted-foreground)", fontSize: 12 }}
							/>
							<YAxis
								allowDecimals={false}
								axisLine={false}
								tickLine={false}
								tickMargin={12}
								width={36}
								tick={{ fill: "var(--muted-foreground)", fontSize: 12 }}
							/>
							<Tooltip
								cursor={{ stroke: "var(--border)", strokeDasharray: "4 6" }}
								content={(props) => (
									<TrendTooltipCard {...props} series={series} />
								)}
							/>
							{series.map((seriesItem) => (
								<Line
									key={seriesItem.key}
									type="monotone"
									dataKey={seriesItem.key}
									name={seriesItem.label}
									stroke={seriesItem.stroke}
									strokeWidth={seriesItem.strokeWidth}
									dot={false}
									activeDot={{
										r: 4,
										fill: "var(--background)",
										stroke: seriesItem.stroke,
										strokeWidth: 2,
									}}
								/>
							))}
						</LineChart>
					</ResponsiveContainer>
				</div>
			</div>

			<div className="grid content-start gap-3 sm:grid-cols-2 xl:grid-cols-1">
				<Card size="sm" className="border-0 shadow-none ring-1 ring-border/70">
					<CardContent className="space-y-1 p-4">
						<CardDescription className="text-xs">
							{averageLabel}
						</CardDescription>
						<p className="text-xl font-semibold tracking-tight">
							{DECIMAL_FORMATTER.format(averageEvents)}
						</p>
					</CardContent>
				</Card>
				<Card size="sm" className="border-0 shadow-none ring-1 ring-border/70">
					<CardContent className="space-y-1 p-4">
						<CardDescription className="text-xs">{latestLabel}</CardDescription>
						<p className="text-xl font-semibold tracking-tight">
							{COUNT_FORMATTER.format(latestReport.total_events)}
						</p>
						<p className="text-xs text-muted-foreground">
							{peakLabel}: {formatTrendDayLabel(peakReport.date)} ·{" "}
							{COUNT_FORMATTER.format(peakReport.total_events)}
						</p>
					</CardContent>
				</Card>
			</div>
		</div>
	);
}

export default function AdminOverviewPage() {
	const { t } = useTranslation(["admin", "tasks"]);
	usePageTitle(t("overview"));
	const timezone = useDisplayTimeZoneStore((s) =>
		resolveActiveDisplayTimeZone(s.preference),
	);
	const trendSeries: TrendSeries[] = [
		{
			badgeClass:
				"border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
			key: "uploads",
			label: t("overview_report_uploads"),
			stroke: "#10b981",
			strokeWidth: 2.5,
		},
		{
			badgeClass:
				"border-sky-500/30 bg-sky-500/10 text-sky-700 dark:text-sky-300",
			key: "shareCreations",
			label: t("overview_report_shares"),
			stroke: "#0ea5e9",
			strokeWidth: 2.5,
		},
		{
			badgeClass:
				"border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300",
			key: "newUsers",
			label: t("overview_report_new_users"),
			stroke: "#f59e0b",
			strokeWidth: 2.5,
		},
	];
	const [overview, setOverview] = useState<AdminOverview | null>(null);
	const [loading, setLoading] = useState(true);
	const [refreshing, setRefreshing] = useState(false);

	const load = useCallback(
		async (mode: "initial" | "refresh" = "initial") => {
			try {
				if (mode === "initial") {
					setLoading(true);
				} else {
					setRefreshing(true);
				}

				const nextOverview = await adminOverviewService.get({
					days: OVERVIEW_TREND_DAYS,
					timezone,
					event_limit: DEFAULT_EVENT_LIMIT,
				});
				setOverview(nextOverview);
			} catch (error) {
				handleApiError(error);
			} finally {
				setLoading(false);
				setRefreshing(false);
			}
		},
		[timezone],
	);

	useEffect(() => {
		void load();
	}, [load]);

	const stats = overview?.stats;
	const statCards = stats
		? [
				{
					label: t("overview_total_users"),
					value: COUNT_FORMATTER.format(stats.total_users),
					icon: "Shield" as const,
					accentClass:
						"bg-blue-100 text-blue-700 dark:bg-blue-950/70 dark:text-blue-300",
				},
				{
					label: t("overview_total_files"),
					value: COUNT_FORMATTER.format(stats.total_files),
					icon: "File" as const,
					accentClass:
						"bg-violet-100 text-violet-700 dark:bg-violet-950/70 dark:text-violet-300",
				},
				{
					label: t("overview_total_blobs"),
					value: COUNT_FORMATTER.format(stats.total_blobs),
					icon: "HardDrive" as const,
					accentClass:
						"bg-slate-200 text-slate-700 dark:bg-slate-800 dark:text-slate-200",
				},
				{
					label: t("overview_total_shares"),
					value: COUNT_FORMATTER.format(stats.total_shares),
					icon: "Link" as const,
					accentClass:
						"bg-cyan-100 text-cyan-700 dark:bg-cyan-950/70 dark:text-cyan-300",
				},
				{
					label: t("overview_total_file_bytes"),
					value: formatBytes(Math.max(stats.total_file_bytes, 0)),
					icon: "Cloud" as const,
					accentClass:
						"bg-fuchsia-100 text-fuchsia-700 dark:bg-fuchsia-950/70 dark:text-fuchsia-300",
				},
				{
					label: t("overview_total_blob_bytes"),
					value: formatBytes(Math.max(stats.total_blob_bytes, 0)),
					icon: "Cloud" as const,
					accentClass:
						"bg-indigo-100 text-indigo-700 dark:bg-indigo-950/70 dark:text-indigo-300",
				},
			]
		: [];

	const formatBackgroundTaskStatus = (status: BackgroundTaskStatus) => {
		switch (status) {
			case "pending":
				return t("tasks:status_pending");
			case "processing":
				return t("tasks:status_processing");
			case "retry":
				return t("tasks:status_retry");
			case "succeeded":
				return t("tasks:status_succeeded");
			case "failed":
				return t("tasks:status_failed");
			case "canceled":
				return t("tasks:status_canceled");
		}
	};

	const formatBackgroundTaskKind = (kind: BackgroundTaskKind) => {
		switch (kind) {
			case "archive_extract":
				return t("tasks:kind_archive_extract");
			case "archive_compress":
				return t("tasks:kind_archive_compress");
			case "thumbnail_generate":
				return t("tasks:kind_thumbnail_generate");
			case "system_runtime":
				return t("tasks:kind_system_runtime");
			default:
				return String(kind).replaceAll("_", " ");
		}
	};

	const formatBackgroundTaskSource = (task: BackgroundTaskEvent) => {
		if (task.team_id != null) {
			return t("overview_background_tasks_source_team", { id: task.team_id });
		}
		if (task.creator_user_id != null) {
			return t("overview_background_tasks_source_user", {
				id: task.creator_user_id,
			});
		}
		return t("overview_background_tasks_source_system");
	};
	const secondaryBadges = stats
		? [
				{
					key: "active-users",
					label: t("overview_active_users_badge", {
						count: stats.active_users,
					}),
				},
				{
					key: "disabled-users",
					label: t("overview_disabled_users_badge", {
						count: stats.disabled_users,
					}),
				},
				{
					key: "today-events",
					label: t("overview_today_events_badge", {
						count: stats.audit_events_today,
					}),
				},
				{
					key: "today-new-users",
					label: t("overview_today_new_users_badge", {
						count: stats.new_users_today,
					}),
				},
				{
					key: "today-uploads",
					label: t("overview_today_uploads_badge", {
						count: stats.uploads_today,
					}),
				},
				{
					key: "today-shares",
					label: t("overview_today_shares_badge", {
						count: stats.shares_today,
					}),
				},
			]
		: [];
	return (
		<AdminLayout>
			<AdminPageShell className="pt-2 md:pt-3">
				<AdminSurface padded={false} className="flex-none overflow-hidden">
					<AdminPageHeader
						title={t("overview")}
						description={t("overview_intro")}
						className="pt-4"
						actions={
							<Button
								variant="outline"
								size="sm"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={() => void load("refresh")}
								disabled={loading || refreshing}
							>
								<Icon
									name={refreshing ? "Spinner" : "ArrowClockwise"}
									className={cn("h-4 w-4", refreshing && "animate-spin")}
								/>
								{t("core:refresh")}
							</Button>
						}
					/>

					{loading && !overview ? (
						<div className={cn("space-y-4 py-4", PAGE_SECTION_PADDING_CLASS)}>
							<div className="space-y-1">
								<Skeleton className="h-5 w-28" />
								<Skeleton className="h-4 w-72" />
							</div>
							<div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_220px]">
								<Skeleton className="h-[320px] w-full rounded-2xl" />
								<div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-1">
									{Array.from({ length: 2 }).map((_, index) => (
										<Skeleton
											// biome-ignore lint/suspicious/noArrayIndexKey: static loading placeholders
											key={`overview-chart-summary-${index}`}
											className="h-24 w-full rounded-xl"
										/>
									))}
								</div>
							</div>
						</div>
					) : overview ? (
						<>
							<div className={cn("py-4", PAGE_SECTION_PADDING_CLASS)}>
								<OverviewTrendChart
									reports={overview.daily_reports}
									emptyTitle={t("overview_daily_trend_empty")}
									emptyDescription={t("overview_daily_trend_empty_desc")}
									averageLabel={t("overview_daily_trend_average")}
									latestLabel={t("overview_daily_trend_latest")}
									peakLabel={t("overview_daily_trend_peak")}
									series={trendSeries}
								/>
							</div>
							<div
								className={cn(
									"flex flex-wrap items-center gap-2 border-t py-3 text-xs text-muted-foreground",
									PAGE_SECTION_PADDING_CLASS,
								)}
							>
								<div className="flex min-w-0 flex-1 flex-wrap items-center gap-2">
									{secondaryBadges.map((badge) => (
										<Badge key={badge.key} variant="secondary">
											{badge.label}
										</Badge>
									))}
								</div>
								<span
									className="ml-auto whitespace-nowrap text-right"
									title={formatDateAbsoluteWithOffset(overview.generated_at)}
								>
									{t("overview_generated_at", {
										date: formatDateAbsolute(overview.generated_at),
									})}
								</span>
							</div>
						</>
					) : (
						<EmptyState
							icon={<Icon name="Presentation" className="h-10 w-10" />}
							title={t("overview_empty_title")}
							description={t("overview_empty_desc")}
							action={
								<Button
									variant="outline"
									size="sm"
									onClick={() => void load("refresh")}
								>
									<Icon name="ArrowClockwise" className="h-4 w-4" />
									{t("core:refresh")}
								</Button>
							}
						/>
					)}
				</AdminSurface>

				<div className="grid gap-4 xl:grid-cols-[minmax(0,1.05fr)_minmax(0,0.95fr)]">
					<AdminSurface padded={false} className="min-h-0 overflow-hidden">
						<div className={cn("border-b py-4", PAGE_SECTION_PADDING_CLASS)}>
							<h3 className="text-base font-semibold">
								{t("overview_summary")}
							</h3>
							<p className="mt-1 text-sm text-muted-foreground">
								{t("overview_summary_desc")}
							</p>
						</div>

						{loading && !overview ? (
							<div
								className={cn(
									"grid gap-3 py-4 sm:grid-cols-2",
									PAGE_SECTION_PADDING_CLASS,
								)}
							>
								{Array.from({ length: 6 }).map((_, index) => (
									<StatCardSkeleton
										// biome-ignore lint/suspicious/noArrayIndexKey: static loading placeholders
										key={`overview-stat-skeleton-${index}`}
									/>
								))}
							</div>
						) : overview ? (
							<div
								className={cn(
									"grid gap-3 py-4 sm:grid-cols-2",
									PAGE_SECTION_PADDING_CLASS,
								)}
							>
								{statCards.map((card) => (
									<StatCard
										key={card.label}
										label={card.label}
										value={card.value}
										icon={card.icon}
										accentClass={card.accentClass}
									/>
								))}
							</div>
						) : (
							<EmptyState
								icon={<Icon name="Presentation" className="h-10 w-10" />}
								title={t("overview_empty_title")}
								description={t("overview_empty_desc")}
							/>
						)}
					</AdminSurface>

					<AdminSurface padded={false} className="min-h-0 overflow-hidden">
						<div className={cn("border-b py-4", PAGE_SECTION_PADDING_CLASS)}>
							<h3 className="text-base font-semibold">
								{t("overview_recent_events")}
							</h3>
							<p className="mt-1 text-sm text-muted-foreground">
								{t("overview_recent_events_desc")}
							</p>
						</div>

						{loading && !overview ? (
							<div className="py-4 md:py-6">
								<SkeletonTable columns={4} rows={8} />
							</div>
						) : overview?.recent_events.length ? (
							<Table>
								<TableHeader>
									<TableRow>
										<TableHead>{t("audit_time")}</TableHead>
										<TableHead>{t("audit_action")}</TableHead>
										<TableHead>{t("audit_user")}</TableHead>
										<TableHead>{t("audit_entity")}</TableHead>
									</TableRow>
								</TableHeader>
								<TableBody>
									{overview.recent_events.map((event) => (
										<TableRow key={event.id}>
											<TableCell
												className="text-xs text-muted-foreground whitespace-nowrap"
												title={formatDateAbsoluteWithOffset(event.created_at)}
											>
												{formatDateAbsolute(event.created_at)}
											</TableCell>
											<TableCell>
												<Badge
													variant="outline"
													className={getActionBadgeClass(event.action)}
												>
													{event.action}
												</Badge>
											</TableCell>
											<TableCell className="text-muted-foreground">
												#{event.user_id}
											</TableCell>
											<TableCell>
												<div className="flex flex-col gap-1">
													<span className="text-sm">
														{event.entity_name ?? event.entity_type ?? "---"}
													</span>
													<span className="text-xs text-muted-foreground">
														{event.entity_type ?? "---"}
													</span>
												</div>
											</TableCell>
										</TableRow>
									))}
								</TableBody>
							</Table>
						) : (
							<EmptyState
								icon={<Icon name="Scroll" className="h-10 w-10" />}
								title={t("overview_recent_events_empty")}
								description={t("overview_recent_events_empty_desc")}
							/>
						)}
					</AdminSurface>
				</div>

				<AdminSurface padded={false} className="min-h-0 overflow-hidden">
					<div className={cn("border-b py-4", PAGE_SECTION_PADDING_CLASS)}>
						<h3 className="text-base font-semibold">
							{t("overview_background_tasks")}
						</h3>
						<p className="mt-1 text-sm text-muted-foreground">
							{t("overview_background_tasks_desc")}
						</p>
					</div>

					{loading && !overview ? (
						<div className="py-4 md:py-6">
							<SkeletonTable columns={5} rows={6} />
						</div>
					) : overview?.recent_background_tasks.length ? (
						<Table>
							<TableHeader>
								<TableRow>
									<TableHead>{t("overview_background_tasks_time")}</TableHead>
									<TableHead>{t("overview_background_tasks_task")}</TableHead>
									<TableHead>{t("overview_background_tasks_status")}</TableHead>
									<TableHead>{t("overview_background_tasks_source")}</TableHead>
									<TableHead>{t("overview_background_tasks_detail")}</TableHead>
								</TableRow>
							</TableHeader>
							<TableBody>
								{overview.recent_background_tasks.map((task) => {
									const duration = formatRuntimeDuration(task.duration_ms);
									const detail = task.last_error ?? task.status_text ?? "---";

									return (
										<TableRow key={task.id}>
											<TableCell
												className="text-xs text-muted-foreground whitespace-nowrap"
												title={formatDateAbsoluteWithOffset(
													backgroundTaskEventTime(task),
												)}
											>
												{formatDateAbsolute(backgroundTaskEventTime(task))}
											</TableCell>
											<TableCell>
												<div className="flex flex-col gap-1">
													<span className="text-sm font-medium">
														{task.display_name}
													</span>
													<span className="text-xs text-muted-foreground">
														{formatBackgroundTaskKind(task.kind)}
														{duration
															? ` · ${t("overview_background_tasks_duration", {
																	duration,
																})}`
															: ""}
													</span>
												</div>
											</TableCell>
											<TableCell>
												<Badge
													variant="outline"
													className={getBackgroundTaskStatusBadgeClass(
														task.status,
													)}
												>
													{formatBackgroundTaskStatus(task.status)}
												</Badge>
											</TableCell>
											<TableCell className="text-sm text-muted-foreground">
												{formatBackgroundTaskSource(task)}
											</TableCell>
											<TableCell className="text-sm text-muted-foreground">
												{detail}
											</TableCell>
										</TableRow>
									);
								})}
							</TableBody>
						</Table>
					) : (
						<EmptyState
							icon={<Icon name="Clock" className="h-10 w-10" />}
							title={t("overview_background_tasks_empty")}
							description={t("overview_background_tasks_empty_desc")}
						/>
					)}
				</AdminSurface>

				<AdminSurface
					padded={false}
					className="flex-none min-h-0 overflow-hidden"
				>
					<div className={cn("border-b py-4", PAGE_SECTION_PADDING_CLASS)}>
						<h3 className="text-base font-semibold">
							{t("overview_daily_reports")}
						</h3>
						<p className="mt-1 text-sm text-muted-foreground">
							{t("overview_daily_reports_desc", {
								days: overview?.days ?? OVERVIEW_TREND_DAYS,
							})}
						</p>
					</div>

					{loading && !overview ? (
						<div className="py-4 md:py-6">
							<SkeletonTable columns={7} rows={7} />
						</div>
					) : (
						<Table>
							<TableHeader>
								<TableRow>
									<TableHead>{t("overview_report_date")}</TableHead>
									<TableHead>{t("overview_report_sign_ins")}</TableHead>
									<TableHead>{t("overview_report_new_users")}</TableHead>
									<TableHead>{t("overview_report_uploads")}</TableHead>
									<TableHead>{t("overview_report_shares")}</TableHead>
									<TableHead>{t("overview_report_deletions")}</TableHead>
									<TableHead>{t("overview_report_total_events")}</TableHead>
								</TableRow>
							</TableHeader>
							<TableBody>
								{overview?.daily_reports.map((report) => (
									<TableRow key={report.date}>
										<TableCell className="font-medium">{report.date}</TableCell>
										<TableCell>{report.sign_ins}</TableCell>
										<TableCell>{report.new_users}</TableCell>
										<TableCell>{report.uploads}</TableCell>
										<TableCell>{report.share_creations}</TableCell>
										<TableCell>{report.deletions}</TableCell>
										<TableCell>{report.total_events}</TableCell>
									</TableRow>
								))}
							</TableBody>
						</Table>
					)}
				</AdminSurface>
			</AdminPageShell>
		</AdminLayout>
	);
}
