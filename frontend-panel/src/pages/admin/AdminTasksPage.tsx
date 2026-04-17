import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { AdminPageHeader } from "@/components/layout/AdminPageHeader";
import { AdminPageShell } from "@/components/layout/AdminPageShell";
import { AdminSurface } from "@/components/layout/AdminSurface";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { useApiList } from "@/hooks/useApiList";
import { usePageTitle } from "@/hooks/usePageTitle";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import { formatDateAbsolute, formatNumber } from "@/lib/format";
import {
	buildOffsetPaginationSearchParams,
	parseOffsetSearchParam,
	parsePageSizeOption,
	parsePageSizeSearchParam,
} from "@/lib/pagination";
import { adminTaskService } from "@/services/adminService";
import type {
	BackgroundTaskKind,
	BackgroundTaskStatus,
	TaskInfo,
} from "@/types/api";

const TASK_PAGE_SIZE_OPTIONS = [20, 50, 100] as const;
const DEFAULT_TASK_PAGE_SIZE = 20 as const;
const TASK_MANAGED_QUERY_KEYS = ["offset", "pageSize"] as const;
const TASK_TEXT_CELL_CONTENT_CLASS =
	"flex min-w-0 items-center rounded-lg bg-muted/10 px-3 py-3 text-left transition-colors duration-200";
const TASK_BADGE_CELL_CONTENT_CLASS =
	"flex items-center rounded-lg bg-muted/20 px-3 py-3 text-left transition-colors duration-200";

function normalizeOffset(offset: number) {
	return Math.max(0, Math.floor(offset));
}

function buildManagedTaskSearchParams({
	offset,
	pageSize,
}: {
	offset: number;
	pageSize: (typeof TASK_PAGE_SIZE_OPTIONS)[number];
}) {
	return buildOffsetPaginationSearchParams({
		offset,
		pageSize,
		defaultPageSize: DEFAULT_TASK_PAGE_SIZE,
	});
}

function getManagedTaskSearchString(searchParams: URLSearchParams) {
	return buildManagedTaskSearchParams({
		offset: normalizeOffset(parseOffsetSearchParam(searchParams.get("offset"))),
		pageSize: parsePageSizeSearchParam(
			searchParams.get("pageSize"),
			TASK_PAGE_SIZE_OPTIONS,
			DEFAULT_TASK_PAGE_SIZE,
		),
	}).toString();
}

function mergeManagedTaskSearchParams(
	searchParams: URLSearchParams,
	managedSearchParams: URLSearchParams,
) {
	const merged = new URLSearchParams(searchParams);
	for (const key of TASK_MANAGED_QUERY_KEYS) {
		merged.delete(key);
	}
	for (const [key, value] of managedSearchParams.entries()) {
		merged.set(key, value);
	}
	return merged;
}

function getTaskStatusBadgeClass(status: BackgroundTaskStatus) {
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

function taskLastActivityAt(task: TaskInfo) {
	return task.finished_at ?? task.updated_at;
}

function taskDetail(task: TaskInfo) {
	return task.last_error ?? task.status_text ?? "-";
}

export default function AdminTasksPage() {
	const { t } = useTranslation(["admin", "tasks", "core"]);
	usePageTitle(t("admin:tasks"));
	const [searchParams, setSearchParams] = useSearchParams();
	const [offset, setOffsetState] = useState(
		normalizeOffset(parseOffsetSearchParam(searchParams.get("offset"))),
	);
	const [pageSize, setPageSize] = useState<
		(typeof TASK_PAGE_SIZE_OPTIONS)[number]
	>(
		parsePageSizeSearchParam(
			searchParams.get("pageSize"),
			TASK_PAGE_SIZE_OPTIONS,
			DEFAULT_TASK_PAGE_SIZE,
		),
	);
	const lastWrittenSearchRef = useRef<string | null>(null);
	const setOffset = (value: number) => {
		setOffsetState(normalizeOffset(value));
	};

	useEffect(() => {
		const managedSearch = getManagedTaskSearchString(searchParams);
		if (managedSearch === lastWrittenSearchRef.current) {
			return;
		}

		const nextOffset = normalizeOffset(
			parseOffsetSearchParam(searchParams.get("offset")),
		);
		const nextPageSize = parsePageSizeSearchParam(
			searchParams.get("pageSize"),
			TASK_PAGE_SIZE_OPTIONS,
			DEFAULT_TASK_PAGE_SIZE,
		);

		setOffsetState((prev) => (prev === nextOffset ? prev : nextOffset));
		setPageSize((prev) => (prev === nextPageSize ? prev : nextPageSize));
	}, [searchParams]);

	useEffect(() => {
		const nextManagedSearchParams = buildManagedTaskSearchParams({
			offset,
			pageSize,
		});
		const nextSearch = nextManagedSearchParams.toString();
		const currentSearch = getManagedTaskSearchString(searchParams);
		if (
			currentSearch !== lastWrittenSearchRef.current &&
			currentSearch !== nextSearch
		) {
			return;
		}

		lastWrittenSearchRef.current = nextSearch;
		if (nextSearch === currentSearch) {
			return;
		}

		setSearchParams(
			mergeManagedTaskSearchParams(searchParams, nextManagedSearchParams),
			{ replace: true },
		);
	}, [offset, pageSize, searchParams, setSearchParams]);

	const { items, loading, reload, total } = useApiList(
		() =>
			adminTaskService.list({
				limit: pageSize,
				offset,
			}),
		[offset, pageSize],
	);

	const totalPages = Math.max(1, Math.ceil(total / pageSize));
	const currentPage = Math.floor(offset / pageSize) + 1;
	const prevPageDisabled = offset === 0;
	const nextPageDisabled = offset + pageSize >= total;
	const pageSizeOptions = TASK_PAGE_SIZE_OPTIONS.map((size) => ({
		label: t("admin:page_size_option", { count: size }),
		value: String(size),
	}));

	const formatTaskStatus = (status: BackgroundTaskStatus) => {
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

	const formatTaskKind = (kind: BackgroundTaskKind) => {
		switch (kind) {
			case "archive_extract":
				return t("tasks:kind_archive_extract");
			case "archive_compress":
				return t("tasks:kind_archive_compress");
			case "system_runtime":
				return t("tasks:kind_system_runtime");
			default:
				return String(kind).replaceAll("_", " ");
		}
	};

	const formatTaskSource = (task: TaskInfo) => {
		if (task.team_id != null) {
			return t("admin:overview_background_tasks_source_team", {
				id: task.team_id,
			});
		}
		if (task.creator_user_id != null) {
			return t("admin:overview_background_tasks_source_user", {
				id: task.creator_user_id,
			});
		}
		return t("admin:overview_background_tasks_source_system");
	};

	const handlePageSizeChange = (value: string | null) => {
		const next = parsePageSizeOption(value, TASK_PAGE_SIZE_OPTIONS);
		if (next == null) return;
		setPageSize(next);
		setOffset(0);
	};

	return (
		<AdminLayout>
			<AdminPageShell>
				<AdminPageHeader
					title={t("admin:tasks")}
					description={t("admin:tasks_intro")}
					actions={
						<Button
							variant="outline"
							size="sm"
							className={ADMIN_CONTROL_HEIGHT_CLASS}
							onClick={() => void reload()}
							disabled={loading}
						>
							<Icon
								name={loading ? "Spinner" : "ArrowsClockwise"}
								className={`mr-1 h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`}
							/>
							{t("admin:refresh")}
						</Button>
					}
				/>

				{loading ? (
					<SkeletonTable columns={8} rows={6} />
				) : items.length === 0 ? (
					<EmptyState
						icon={<Icon name="Clock" className="h-10 w-10" />}
						title={t("admin:no_tasks")}
						description={t("admin:no_tasks_desc")}
					/>
				) : (
					<AdminSurface padded={false}>
						<ScrollArea className="min-h-0 flex-1">
							<Table>
								<TableHeader>
									<TableRow>
										<TableHead className="w-16">{t("admin:id")}</TableHead>
										<TableHead className="min-w-[240px]">
											{t("admin:task_name")}
										</TableHead>
										<TableHead className="w-[180px]">
											{t("core:type")}
										</TableHead>
										<TableHead className="w-[160px]">
											{t("core:status")}
										</TableHead>
										<TableHead className="w-[160px]">
											{t("admin:task_source")}
										</TableHead>
										<TableHead className="w-[160px]">
											{t("admin:task_progress")}
										</TableHead>
										<TableHead className="w-[180px]">
											{t("admin:task_last_activity")}
										</TableHead>
										<TableHead className="min-w-[240px]">
											{t("admin:task_detail")}
										</TableHead>
									</TableRow>
								</TableHeader>
								<TableBody>
									{items.map((task) => (
										<TableRow key={task.id}>
											<TableCell>
												<div className={TASK_TEXT_CELL_CONTENT_CLASS}>
													<span className="font-mono text-xs text-muted-foreground">
														{task.id}
													</span>
												</div>
											</TableCell>
											<TableCell>
												<div className="flex min-w-0 flex-col gap-2 rounded-lg bg-muted/10 px-3 py-3 text-left">
													<span className="truncate text-sm font-medium text-foreground">
														{task.display_name}
													</span>
													<span className="font-mono text-xs text-muted-foreground">
														#{task.id}
													</span>
												</div>
											</TableCell>
											<TableCell>
												<div className={TASK_BADGE_CELL_CONTENT_CLASS}>
													<Badge variant="outline">
														{formatTaskKind(task.kind)}
													</Badge>
												</div>
											</TableCell>
											<TableCell>
												<div className={TASK_BADGE_CELL_CONTENT_CLASS}>
													<span
														className={`inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium ${getTaskStatusBadgeClass(task.status)}`}
													>
														{formatTaskStatus(task.status)}
													</span>
												</div>
											</TableCell>
											<TableCell>
												<div className={TASK_TEXT_CELL_CONTENT_CLASS}>
													<span className="text-xs text-muted-foreground">
														{formatTaskSource(task)}
													</span>
												</div>
											</TableCell>
											<TableCell>
												<div className="flex min-w-0 flex-col rounded-lg bg-muted/10 px-3 py-3 text-left">
													<span className="text-sm font-medium text-foreground">
														{task.progress_percent}%
													</span>
													<span className="text-xs text-muted-foreground">
														{formatNumber(task.progress_current)}
														{task.progress_total > 0
															? ` / ${formatNumber(task.progress_total)}`
															: ""}
													</span>
												</div>
											</TableCell>
											<TableCell>
												<div className={TASK_TEXT_CELL_CONTENT_CLASS}>
													<span className="text-xs text-muted-foreground whitespace-nowrap">
														{formatDateAbsolute(taskLastActivityAt(task))}
													</span>
												</div>
											</TableCell>
											<TableCell>
												<div className={TASK_TEXT_CELL_CONTENT_CLASS}>
													<span
														className="truncate text-xs text-muted-foreground"
														title={taskDetail(task)}
													>
														{taskDetail(task)}
													</span>
												</div>
											</TableCell>
										</TableRow>
									))}
								</TableBody>
							</Table>
						</ScrollArea>
					</AdminSurface>
				)}

				{total > 0 ? (
					<div className="flex items-center justify-between gap-3 px-4 pb-4 text-sm text-muted-foreground md:px-6">
						<div className="flex items-center gap-3">
							<span>
								{t("admin:entries_page", {
									total,
									current: currentPage,
									pages: totalPages,
								})}
							</span>
							<Select
								items={pageSizeOptions}
								value={String(pageSize)}
								onValueChange={handlePageSizeChange}
							>
								<SelectTrigger width="page-size">
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									{pageSizeOptions.map((option) => (
										<SelectItem key={option.value} value={option.value}>
											{option.label}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
						</div>
						<TooltipProvider>
							<div className="flex items-center gap-2">
								<Tooltip>
									<TooltipTrigger
										render={
											<Button
												variant="outline"
												size="sm"
												disabled={prevPageDisabled}
												onClick={() =>
													setOffset(Math.max(0, offset - pageSize))
												}
											/>
										}
									>
										<Icon name="CaretLeft" className="h-4 w-4" />
									</TooltipTrigger>
									{prevPageDisabled ? (
										<TooltipContent>
											{t("admin:pagination_prev_disabled")}
										</TooltipContent>
									) : null}
								</Tooltip>
								<Tooltip>
									<TooltipTrigger
										render={
											<Button
												variant="outline"
												size="sm"
												disabled={nextPageDisabled}
												onClick={() => setOffset(offset + pageSize)}
											/>
										}
									>
										<Icon name="CaretRight" className="h-4 w-4" />
									</TooltipTrigger>
									{nextPageDisabled ? (
										<TooltipContent>
											{t("admin:pagination_next_disabled")}
										</TooltipContent>
									) : null}
								</Tooltip>
							</div>
						</TooltipProvider>
					</div>
				) : null}
			</AdminPageShell>
		</AdminLayout>
	);
}
