import { lazy, Suspense } from "react";
import { EmptyState } from "@/components/common/EmptyState";
import { Icon } from "@/components/ui/icon";
import type { AdminOverview } from "@/types/api";

const OverviewTrendChartContent = lazy(() =>
	import("./OverviewTrendChartContent").then((module) => ({
		default: module.OverviewTrendChartContent,
	})),
);

export type DailyOverviewReport = AdminOverview["daily_reports"][number];
export type TrendSeriesKey = "newUsers" | "shareCreations" | "uploads";

export interface OverviewTrendSeries {
	badgeClass: string;
	key: TrendSeriesKey;
	label: string;
	stroke: string;
	strokeWidth: number;
}

interface OverviewTrendChartProps {
	reports: DailyOverviewReport[];
	emptyTitle: string;
	emptyDescription: string;
	averageLabel: string;
	latestLabel: string;
	peakLabel: string;
	series: OverviewTrendSeries[];
}

export function OverviewTrendChart({
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
				icon={<Icon name="Presentation" className="size-10" />}
				title={emptyTitle}
				description={emptyDescription}
			/>
		);
	}

	return (
		<Suspense fallback={<div className="min-h-[280px]" />}>
			<OverviewTrendChartContent
				reports={reports}
				averageLabel={averageLabel}
				latestLabel={latestLabel}
				peakLabel={peakLabel}
				series={series}
			/>
		</Suspense>
	);
}
