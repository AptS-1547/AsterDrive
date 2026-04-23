import {
	type ReactNode,
	useCallback,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { EmptyState } from "@/components/common/EmptyState";
import { AppLayout } from "@/components/layout/AppLayout";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Icon } from "@/components/ui/icon";
import { Progress } from "@/components/ui/progress";
import { handleApiError } from "@/hooks/useApiError";
import { usePageTitle } from "@/hooks/usePageTitle";
import { PAGE_SECTION_PADDING_CLASS } from "@/lib/constants";
import { formatDateAbsolute, formatNumber } from "@/lib/format";
import { cn } from "@/lib/utils";
import { workspaceFolderPath } from "@/lib/workspace";
import { taskService } from "@/services/taskService";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import type {
	BackgroundTaskKind,
	BackgroundTaskStatus,
	TaskInfo,
	TaskStepInfo,
	TaskStepStatus,
} from "@/types/api";

const PAGE_SIZE = 20;
const TASK_POLL_INTERVAL_MS = 3000;
const ACTIVE_TASK_STATUSES = new Set<BackgroundTaskStatus>([
	"pending",
	"processing",
	"retry",
]);
const TASK_DETAILS_EXPAND_DURATION_MS = 280;
const TASK_DETAILS_COLLAPSE_DURATION_MS = 240;
const TASK_DETAILS_EXPAND_EASING = "cubic-bezier(0.22, 1, 0.36, 1)";
const TASK_DETAILS_COLLAPSE_EASING = "cubic-bezier(0.32, 0, 0.67, 0.96)";

function statusBadgeVariant(status: BackgroundTaskStatus) {
	switch (status) {
		case "pending":
		case "processing":
		case "retry":
			return "secondary";
		case "succeeded":
			return "default";
		case "failed":
			return "destructive";
		case "canceled":
			return "outline";
	}
}

function taskMetaTextClass(status: BackgroundTaskStatus) {
	switch (status) {
		case "processing":
		case "retry":
			return "text-primary";
		case "succeeded":
			return "text-foreground";
		case "failed":
			return "text-destructive";
		case "pending":
		case "canceled":
			return "text-muted-foreground";
	}
}

function stepStatusTextClass(status: TaskStepStatus) {
	switch (status) {
		case "active":
			return "text-primary";
		case "succeeded":
			return "text-foreground";
		case "failed":
			return "text-destructive";
		case "canceled":
		case "pending":
			return "text-muted-foreground";
	}
}

function stepProgressPercent(step: TaskStepInfo) {
	if (step.progress_total <= 0) {
		return step.status === "succeeded" ? 100 : 0;
	}
	return Math.max(
		0,
		Math.min(
			100,
			Math.floor((step.progress_current * 100) / step.progress_total),
		),
	);
}

function stepConnectorClass(status: TaskStepStatus) {
	switch (status) {
		case "succeeded":
			return "bg-primary/70";
		case "active":
			return "bg-primary/35";
		case "failed":
			return "bg-destructive/35";
		case "canceled":
			return "bg-border/60";
		case "pending":
			return "bg-border/40";
	}
}

function stepCircleClass(status: TaskStepStatus) {
	switch (status) {
		case "active":
			return "border-primary bg-primary text-primary-foreground shadow-[0_0_0_4px_rgba(249,115,22,0.16)]";
		case "succeeded":
			return "border-primary/40 bg-primary/12 text-foreground";
		case "failed":
			return "border-destructive/50 bg-destructive/10 text-destructive";
		case "canceled":
			return "border-border/70 bg-muted/35 text-muted-foreground";
		case "pending":
			return "border-border/60 bg-background/90 text-muted-foreground";
	}
}

function stepCircleLabel(index: number, status: TaskStepStatus) {
	switch (status) {
		case "failed":
			return "!";
		case "canceled":
			return "X";
		default:
			return String(index + 1);
	}
}

function currentTaskStep(task: TaskInfo) {
	return (
		task.steps.find((step) => step.status === "active") ??
		task.steps.find((step) => step.status === "failed") ??
		task.steps[task.steps.length - 1] ??
		null
	);
}

function AnimatedTaskDetails({
	children,
	className,
	open,
}: {
	children: ReactNode;
	className?: string;
	open: boolean;
}) {
	const containerRef = useRef<HTMLDivElement | null>(null);
	const contentRef = useRef<HTMLDivElement | null>(null);
	const [mounted, setMounted] = useState(open);

	useEffect(() => {
		if (typeof window === "undefined") {
			setMounted(open);
			return;
		}

		if (open) {
			setMounted(true);
		}
	}, [open]);

	useLayoutEffect(() => {
		if (typeof window === "undefined" || !mounted) {
			return;
		}

		const container = containerRef.current;
		const content = contentRef.current;
		if (!container || !content) {
			return;
		}

		const prefersReducedMotion =
			typeof window.matchMedia === "function" &&
			window.matchMedia("(prefers-reduced-motion: reduce)").matches;
		const duration = prefersReducedMotion
			? 0
			: open
				? TASK_DETAILS_EXPAND_DURATION_MS
				: TASK_DETAILS_COLLAPSE_DURATION_MS;
		let frameA: number | null = null;
		let frameB: number | null = null;
		let timer: number | null = null;
		const fullHeight = `${content.scrollHeight}px`;

		container.style.overflow = "hidden";
		container.style.transitionProperty = "max-height, opacity, transform";
		container.style.transitionDuration = `${duration}ms`;
		container.style.transitionTimingFunction = open
			? TASK_DETAILS_EXPAND_EASING
			: TASK_DETAILS_COLLAPSE_EASING;

		if (open) {
			container.style.maxHeight = "0px";
			container.style.opacity = "0";
			container.style.transform = "translateY(-6px)";
			frameA = window.requestAnimationFrame(() => {
				frameB = window.requestAnimationFrame(() => {
					container.style.maxHeight = fullHeight;
					container.style.opacity = "1";
					container.style.transform = "translateY(0)";
				});
			});
			timer = window.setTimeout(() => {
				container.style.maxHeight = "none";
				container.style.opacity = "1";
				container.style.transform = "translateY(0)";
			}, duration);
		} else {
			container.style.maxHeight = fullHeight;
			container.style.opacity = "1";
			container.style.transform = "translateY(0)";
			frameA = window.requestAnimationFrame(() => {
				container.style.maxHeight = "0px";
				container.style.opacity = "0";
				container.style.transform = "translateY(-6px)";
			});
			timer = window.setTimeout(() => {
				setMounted(false);
			}, duration);
		}

		return () => {
			if (frameA !== null) {
				window.cancelAnimationFrame(frameA);
			}
			if (frameB !== null) {
				window.cancelAnimationFrame(frameB);
			}
			if (timer !== null) {
				window.clearTimeout(timer);
			}
		};
	}, [mounted, open]);

	if (!mounted) {
		return null;
	}

	return (
		<div
			ref={containerRef}
			aria-hidden={!open}
			className={cn("overflow-hidden", className)}
		>
			<div ref={contentRef} className="min-h-0">
				{children}
			</div>
		</div>
	);
}

export default function TasksPage() {
	const { t } = useTranslation(["core", "tasks"]);
	const navigate = useNavigate();
	const workspace = useWorkspaceStore((s) => s.workspace);
	usePageTitle(t("tasks:title"));
	const [page, setPage] = useState(0);
	const [loading, setLoading] = useState(true);
	const [tasks, setTasks] = useState<TaskInfo[]>([]);
	const [total, setTotal] = useState(0);
	const [expandedTaskIds, setExpandedTaskIds] = useState<Set<number>>(
		() => new Set(),
	);
	const [retryingTaskId, setRetryingTaskId] = useState<number | null>(null);

	const loadPage = useCallback(
		async (targetPage: number, options?: { silent?: boolean }) => {
			const silent = options?.silent ?? false;
			try {
				if (!silent) {
					setLoading(true);
				}
				const data = await taskService.listInWorkspace({
					limit: PAGE_SIZE,
					offset: targetPage * PAGE_SIZE,
				});
				setTasks(data.items);
				setTotal(data.total);
				return data;
			} catch (error) {
				if (!silent) {
					handleApiError(error);
				}
				return null;
			} finally {
				if (!silent) {
					setLoading(false);
				}
			}
		},
		[],
	);

	useEffect(() => {
		void loadPage(page);
	}, [loadPage, page]);

	const hasActiveTasks = useMemo(
		() => tasks.some((task) => ACTIVE_TASK_STATUSES.has(task.status)),
		[tasks],
	);

	useEffect(() => {
		if (!hasActiveTasks) {
			return;
		}

		const timer = window.setInterval(() => {
			void loadPage(page, { silent: true });
		}, TASK_POLL_INTERVAL_MS);

		return () => window.clearInterval(timer);
	}, [hasActiveTasks, loadPage, page]);

	useEffect(() => {
		setExpandedTaskIds((prev) => {
			if (prev.size === 0) {
				return prev;
			}

			const visibleTaskIds = new Set(tasks.map((task) => task.id));
			const next = new Set<number>();
			let changed = false;

			for (const taskId of prev) {
				if (visibleTaskIds.has(taskId)) {
					next.add(taskId);
				} else {
					changed = true;
				}
			}

			return changed ? next : prev;
		});
	}, [tasks]);

	const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

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
			case "thumbnail_generate":
				return t("tasks:kind_thumbnail_generate");
			case "system_runtime":
				return t("tasks:kind_system_runtime");
			default:
				return String(kind).replaceAll("_", " ");
		}
	};

	const formatTaskStepStatus = (status: TaskStepStatus) => {
		switch (status) {
			case "pending":
				return t("tasks:step_status_pending");
			case "active":
				return t("tasks:step_status_active");
			case "succeeded":
				return t("tasks:step_status_succeeded");
			case "failed":
				return t("tasks:step_status_failed");
			case "canceled":
				return t("tasks:step_status_canceled");
		}
	};

	const formatTaskStepTitle = (
		taskKind: BackgroundTaskKind,
		step: TaskStepInfo,
	) => {
		const key = `tasks:step_${taskKind}_${step.key}`;
		const translated = t(key);
		return translated === key ? step.title : translated;
	};

	const handleRetry = useCallback(
		async (taskId: number) => {
			try {
				setRetryingTaskId(taskId);
				await taskService.retryTask(taskId);
				toast.success(t("tasks:retry_success"));
				await loadPage(page, { silent: true });
			} catch (error) {
				handleApiError(error);
			} finally {
				setRetryingTaskId(null);
			}
		},
		[loadPage, page, t],
	);

	const toggleTaskDetails = useCallback((taskId: number) => {
		setExpandedTaskIds((prev) => {
			const next = new Set(prev);
			if (next.has(taskId)) {
				next.delete(taskId);
			} else {
				next.add(taskId);
			}
			return next;
		});
	}, []);

	const formatProgressCounts = (current: number, total: number) =>
		`${formatNumber(current)} / ${formatNumber(total)}`;

	const renderTaskSummaryTimestamp = (task: TaskInfo) => {
		switch (task.status) {
			case "pending":
				return t("tasks:summary_created_at", {
					date: formatDateAbsolute(task.created_at),
				});
			case "processing":
			case "retry":
				if (task.started_at) {
					return t("tasks:summary_started_at", {
						date: formatDateAbsolute(task.started_at),
					});
				}
				return t("tasks:summary_created_at", {
					date: formatDateAbsolute(task.created_at),
				});
			case "succeeded":
				if (task.finished_at) {
					return t("tasks:summary_finished_at", {
						date: formatDateAbsolute(task.finished_at),
					});
				}
				if (task.started_at) {
					return t("tasks:summary_started_at", {
						date: formatDateAbsolute(task.started_at),
					});
				}
				return t("tasks:summary_created_at", {
					date: formatDateAbsolute(task.created_at),
				});
			case "failed":
				if (task.finished_at) {
					return t("tasks:summary_failed_at", {
						date: formatDateAbsolute(task.finished_at),
					});
				}
				if (task.started_at) {
					return t("tasks:summary_started_at", {
						date: formatDateAbsolute(task.started_at),
					});
				}
				return t("tasks:summary_created_at", {
					date: formatDateAbsolute(task.created_at),
				});
			case "canceled":
				if (task.finished_at) {
					return t("tasks:summary_canceled_at", {
						date: formatDateAbsolute(task.finished_at),
					});
				}
				if (task.started_at) {
					return t("tasks:summary_started_at", {
						date: formatDateAbsolute(task.started_at),
					});
				}
				return t("tasks:summary_created_at", {
					date: formatDateAbsolute(task.created_at),
				});
		}
	};

	const buildTaskTimeline = (task: TaskInfo) => {
		const timeline = [
			{
				label: t("tasks:timeline_created_label"),
				value: formatDateAbsolute(task.created_at),
			},
		];

		if (task.started_at) {
			timeline.push({
				label: t("tasks:timeline_started_label"),
				value: formatDateAbsolute(task.started_at),
			});
		}

		if (task.finished_at) {
			const labelKey =
				task.status === "failed"
					? "tasks:timeline_failed_label"
					: task.status === "canceled"
						? "tasks:timeline_canceled_label"
						: "tasks:timeline_finished_label";
			timeline.push({
				label: t(labelKey),
				value: formatDateAbsolute(task.finished_at),
			});
		}

		return timeline;
	};

	const parseTaskResult = (task: TaskInfo) => {
		if (!task.result) {
			return null;
		}

		switch (task.result.kind) {
			case "archive_compress":
				return {
					target_folder_id: task.result.target_folder_id ?? null,
					target_path: task.result.target_path,
				};
			case "archive_extract":
				return {
					target_folder_id: task.result.target_folder_id,
					target_path: task.result.target_path,
				};
			default:
				return null;
		}
	};

	const openTaskTargetFolder = useCallback(
		(targetFolderId: number | null) => {
			navigate(workspaceFolderPath(workspace, targetFolderId), {
				viewTransition: false,
			});
		},
		[navigate, workspace],
	);

	return (
		<AppLayout>
			<div className="flex min-h-0 flex-1 flex-col overflow-auto">
				<div
					className={`mx-auto flex w-full max-w-6xl flex-col gap-5 py-4 md:py-6 ${PAGE_SECTION_PADDING_CLASS}`}
				>
					<div className="flex flex-wrap items-center gap-3">
						<h1 className="text-2xl font-semibold tracking-tight">
							{t("tasks:title")}
						</h1>
						<Button
							variant="ghost"
							size="icon-sm"
							onClick={() => void loadPage(page)}
							disabled={loading}
							aria-label={t("core:refresh")}
							title={t("core:refresh")}
						>
							<Icon
								name={loading ? "Spinner" : "ArrowsClockwise"}
								className={`h-4 w-4 ${loading ? "animate-spin" : ""}`}
							/>
						</Button>
						{hasActiveTasks ? (
							<span className="text-sm text-muted-foreground">
								{t("tasks:active_polling_hint")}
							</span>
						) : null}
					</div>

					{loading ? (
						<div className="space-y-3">
							{["task-s1", "task-s2", "task-s3"].map((key) => (
								<Card key={key} className="h-48 animate-pulse bg-muted/20" />
							))}
						</div>
					) : tasks.length === 0 ? (
						<Card className="bg-muted/15">
							<div className="py-12">
								<EmptyState
									icon={<Icon name="Clock" className="h-10 w-10" />}
									title={t("tasks:empty_title")}
									description={t("tasks:empty_desc")}
								/>
							</div>
						</Card>
					) : (
						<div className="space-y-3">
							{tasks.map((task) => {
								const parsedResult = parseTaskResult(task);
								const activeStep = currentTaskStep(task);
								const taskTimeline = buildTaskTimeline(task);
								const activeStepDetail = activeStep?.detail?.trim() ?? null;
								const statusText = task.status_text?.trim() ?? null;
								const summaryTimestamp = renderTaskSummaryTimestamp(task);
								const detailsExpanded = expandedTaskIds.has(task.id);
								const detailsSectionId = `task-details-${task.id}`;
								const hasExpandableDetails =
									task.steps.length > 0 ||
									task.last_error !== null ||
									(task.status === "succeeded" && parsedResult !== null);
								const taskSummaryText =
									statusText && activeStepDetail
										? statusText.toLocaleLowerCase() ===
											activeStepDetail.toLocaleLowerCase()
											? activeStepDetail
											: statusText
										: statusText || activeStepDetail;
								const progressValue =
									task.status === "succeeded"
										? 100
										: Math.max(0, Math.min(100, task.progress_percent));
								const progressText =
									task.progress_total > 0
										? t("tasks:progress_ratio", {
												current: task.progress_current,
												total: task.progress_total,
											})
										: null;

								return (
									<Card key={task.id} className="p-4 md:p-5">
										<div className="flex flex-col gap-4">
											<div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
												<div className="min-w-0 space-y-2">
													<div className="flex flex-wrap items-center gap-2">
														<h2 className="truncate text-lg font-semibold">
															{task.display_name}
														</h2>
														<Badge variant={statusBadgeVariant(task.status)}>
															{formatTaskStatus(task.status)}
														</Badge>
														<Badge variant="outline">
															{formatTaskKind(task.kind)}
														</Badge>
													</div>
													<div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
														<span className="text-muted-foreground">
															{t("tasks:task_id_label", { id: task.id })}
														</span>
														{summaryTimestamp ? (
															<>
																<span className="text-border">·</span>
																<span
																	className={`font-medium ${taskMetaTextClass(task.status)}`}
																>
																	{summaryTimestamp}
																</span>
															</>
														) : null}
													</div>
												</div>
												<div className="flex shrink-0 items-center gap-2">
													{task.status === "succeeded" && parsedResult ? (
														<Button
															variant="outline"
															size="sm"
															onClick={() =>
																openTaskTargetFolder(
																	parsedResult.target_folder_id ?? null,
																)
															}
														>
															<Icon
																name="FolderOpen"
																className="mr-1 h-4 w-4"
															/>
															{t("tasks:open_target_folder")}
														</Button>
													) : null}
													{hasExpandableDetails ? (
														<Button
															variant="outline"
															size="sm"
															aria-controls={detailsSectionId}
															aria-expanded={detailsExpanded}
															onClick={() => toggleTaskDetails(task.id)}
														>
															<Icon
																name={detailsExpanded ? "CaretUp" : "CaretDown"}
																className="mr-1 h-4 w-4"
															/>
															{detailsExpanded
																? t("tasks:hide_details")
																: t("tasks:show_details")}
														</Button>
													) : null}
													{task.can_retry ? (
														<Button
															variant="outline"
															size="sm"
															onClick={() => void handleRetry(task.id)}
															disabled={retryingTaskId === task.id}
														>
															<Icon
																name={
																	retryingTaskId === task.id
																		? "Spinner"
																		: "ArrowCounterClockwise"
																}
																className={`mr-1 h-4 w-4 ${
																	retryingTaskId === task.id
																		? "animate-spin"
																		: ""
																}`}
															/>
															{t("tasks:retry_task")}
														</Button>
													) : null}
												</div>
											</div>

											<div className="space-y-2">
												<div className="flex flex-wrap items-center justify-between gap-2 text-sm">
													<span className="text-muted-foreground">
														{t("tasks:progress_label")}
													</span>
													<span className="font-medium tabular-nums">
														{progressValue}%
													</span>
												</div>
												<Progress value={progressValue} className="h-2" />
												{taskSummaryText ? (
													<p className="text-sm text-muted-foreground">
														{t("tasks:status_text_label")}: {taskSummaryText}
													</p>
												) : null}
											</div>

											<AnimatedTaskDetails
												open={detailsExpanded}
												className="space-y-2.5"
											>
												<div
													id={detailsSectionId}
													className="space-y-2.5 pt-0.5"
												>
													<div className="rounded-lg border bg-background/70 px-3 py-3">
														<div className="grid gap-3 lg:grid-cols-[minmax(0,1.35fr)_minmax(16rem,0.65fr)] lg:items-start">
															<div className="space-y-2">
																<div className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
																	{t("tasks:timeline_label")}
																</div>
																<div className="flex flex-wrap gap-2">
																	{taskTimeline.map((entry) => (
																		<div
																			key={`${task.id}-${entry.label}`}
																			className="min-w-[11rem] flex-1 rounded-md bg-muted/25 px-2.5 py-2"
																		>
																			<div className="text-[11px] font-medium uppercase tracking-[0.12em] text-muted-foreground">
																				{entry.label}
																			</div>
																			<div className="mt-1 text-sm font-medium tabular-nums text-foreground">
																				{entry.value}
																			</div>
																		</div>
																	))}
																</div>
															</div>
															<div className="space-y-2 rounded-lg bg-muted/20 px-3 py-2.5">
																<div className="flex items-end justify-between gap-3">
																	<div className="space-y-1">
																		<div className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
																			{t("tasks:progress_label")}
																		</div>
																		<div className="text-3xl font-semibold tracking-tight tabular-nums">
																			{progressValue}%
																		</div>
																	</div>
																	{progressText ? (
																		<div className="text-right text-xs text-muted-foreground">
																			<div>
																				{t("tasks:progress_ratio_label")}
																			</div>
																			<div className="font-medium text-foreground tabular-nums">
																				{formatProgressCounts(
																					task.progress_current,
																					task.progress_total,
																				)}
																			</div>
																		</div>
																	) : null}
																</div>
																<Progress
																	value={progressValue}
																	className="h-2"
																/>
															</div>
														</div>
													</div>

													{task.steps.length > 0 ? (
														<div className="space-y-2.5 rounded-lg border bg-muted/15 px-3 py-3">
															<div className="flex flex-wrap items-start justify-between gap-2">
																<div className="text-sm font-medium">
																	{t("tasks:steps_label")}
																</div>
																{activeStep && activeStep.progress_total > 0 ? (
																	<div className="text-right text-xs text-muted-foreground">
																		<div>{t("tasks:step_progress_label")}</div>
																		<div className="font-medium tabular-nums text-foreground">
																			{stepProgressPercent(activeStep)}% ·{" "}
																			{formatProgressCounts(
																				activeStep.progress_current,
																				activeStep.progress_total,
																			)}
																		</div>
																	</div>
																) : null}
															</div>
															<div className="overflow-x-auto pb-0.5">
																<div className="w-full">
																	<div className="mx-auto flex w-fit min-w-max items-start px-0.5 py-1.5">
																		{task.steps.map((step, index) => (
																			<div
																				key={`${task.id}-${step.key}`}
																				className="contents"
																			>
																				<div className="w-32 shrink-0 md:w-36 lg:w-40">
																					<div className="flex flex-col items-center text-center">
																						<div className="relative flex h-10 w-10 items-center justify-center md:h-11 md:w-11">
																							{step.status === "active" ? (
																								<span className="absolute inset-0 rounded-full border-2 border-primary/20 border-t-primary animate-spin" />
																							) : null}
																							<span
																								className={`relative flex h-8 w-8 items-center justify-center rounded-full border text-xs font-semibold transition-colors md:h-9 md:w-9 md:text-sm ${stepCircleClass(step.status)}`}
																							>
																								{stepCircleLabel(
																									index,
																									step.status,
																								)}
																							</span>
																						</div>
																						<div className="mt-2 space-y-0.5 md:mt-2.5">
																							<p className="text-xs font-semibold leading-snug md:text-sm">
																								{index + 1}.{" "}
																								{formatTaskStepTitle(
																									task.kind,
																									step,
																								)}
																							</p>
																							<p
																								className={`text-[11px] font-medium ${stepStatusTextClass(step.status)}`}
																							>
																								{formatTaskStepStatus(
																									step.status,
																								)}
																							</p>
																						</div>
																					</div>
																				</div>
																				{index < task.steps.length - 1 ? (
																					<div className="flex h-10 w-10 shrink-0 items-center px-1 md:h-11 md:w-12 md:px-1.5">
																						<div
																							className={`h-1 w-full rounded-full ${stepConnectorClass(step.status)}`}
																						/>
																					</div>
																				) : null}
																			</div>
																		))}
																	</div>
																</div>
															</div>
														</div>
													) : null}

													{task.last_error ? (
														<div className="rounded-lg border border-destructive/20 bg-destructive/5 px-3 py-2 text-sm text-destructive">
															<span className="font-medium">
																{t("tasks:error_label")}:
															</span>{" "}
															{task.last_error}
														</div>
													) : null}

													{task.status === "succeeded" && parsedResult ? (
														<div className="rounded-lg border bg-muted/20 px-3 py-3 text-sm">
															<div className="min-w-0">
																<div className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
																	{t("tasks:result_path_label")}
																</div>
																<div className="mt-1 truncate text-foreground">
																	{parsedResult.target_path}
																</div>
															</div>
														</div>
													) : null}
												</div>
											</AnimatedTaskDetails>
										</div>
									</Card>
								);
							})}
						</div>
					)}

					{tasks.length > 0 ? (
						<div className="flex flex-wrap items-center justify-between gap-3 text-sm text-muted-foreground">
							<span>
								{t("tasks:pagination_desc", {
									current: page + 1,
									total: totalPages,
									count: total,
								})}
							</span>
							<div className="flex items-center gap-2">
								<Button
									variant="outline"
									size="sm"
									onClick={() => setPage((current) => Math.max(0, current - 1))}
									disabled={page === 0}
								>
									{t("tasks:prev_page")}
								</Button>
								<Button
									variant="outline"
									size="sm"
									onClick={() =>
										setPage((current) => Math.min(totalPages - 1, current + 1))
									}
									disabled={page >= totalPages - 1}
								>
									{t("tasks:next_page")}
								</Button>
							</div>
						</div>
					) : null}
				</div>
			</div>
		</AppLayout>
	);
}
