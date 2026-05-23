import { useTranslation } from "react-i18next";
import { EmptyState } from "@/components/common/EmptyState";
import { AdminSurface } from "@/components/layout/AdminSurface";
import { Card, CardContent, CardDescription } from "@/components/ui/card";
import { Icon, type IconName } from "@/components/ui/icon";
import { Skeleton } from "@/components/ui/skeleton";
import { PAGE_SECTION_PADDING_CLASS } from "@/lib/constants";
import { formatBytes } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { AdminOverview } from "@/types/api";
import { COUNT_FORMATTER } from "./overviewPresentation";

interface StatCardProps {
	accentClass: string;
	icon: IconName;
	label: string;
	value: string;
}

function StatCard({ accentClass, icon, label, value }: StatCardProps) {
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
						"mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-xl",
						accentClass,
					)}
				>
					<Icon name={icon} className="size-4" />
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
				<Skeleton className="size-9 rounded-xl" />
			</CardContent>
		</Card>
	);
}

interface OverviewStatsSectionProps {
	loading: boolean;
	overview: AdminOverview | null;
}

export function OverviewStatsSection({
	loading,
	overview,
}: OverviewStatsSectionProps) {
	const { t } = useTranslation("admin");
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

	return (
		<AdminSurface padded={false} className="min-h-0 overflow-hidden">
			<div className={cn("border-b py-4", PAGE_SECTION_PADDING_CLASS)}>
				<h3 className="text-base font-semibold">{t("overview_summary")}</h3>
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
					icon={<Icon name="Presentation" className="size-10" />}
					title={t("overview_empty_title")}
					description={t("overview_empty_desc")}
				/>
			)}
		</AdminSurface>
	);
}
