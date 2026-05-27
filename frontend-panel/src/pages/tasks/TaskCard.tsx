import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Icon } from "@/components/ui/icon";
import type { TaskInfo } from "@/types/api";
import { AnimatedTaskDetails } from "./AnimatedTaskDetails";
import {
	TaskDetailsContent,
	TaskStepsPreview,
	taskHasExpandableDetails,
} from "./TaskDetailsPanel";
import {
	currentTaskStep,
	formatTaskKind,
	formatTaskStatus,
	parseTaskResult,
	statusBadgeVariant,
	taskMetaTextClass,
	taskSummaryTimestamp,
} from "./taskPresentation";

interface TaskCardProps {
	detailsExpanded: boolean;
	onOpenTargetFolder: (targetFolderId: number | null) => void;
	onRetry: (taskId: number) => void;
	onToggleDetails: (taskId: number) => void;
	retrying: boolean;
	task: TaskInfo;
}

export function TaskCard({
	detailsExpanded,
	onOpenTargetFolder,
	onRetry,
	onToggleDetails,
	retrying,
	task,
}: TaskCardProps) {
	const { t } = useTranslation(["core", "tasks"]);
	const parsedResult = parseTaskResult(task);
	const activeStep = currentTaskStep(task);
	const activeStepDetail = activeStep?.detail?.trim() ?? null;
	const statusText = task.status_text?.trim() ?? null;
	const summaryTimestamp = taskSummaryTimestamp(t, task);
	const detailsSectionId = `task-details-${task.id}`;
	const hasExpandableDetails = taskHasExpandableDetails(task);
	const taskSummaryText =
		statusText && activeStepDetail
			? statusText.toLocaleLowerCase() === activeStepDetail.toLocaleLowerCase()
				? activeStepDetail
				: statusText
			: statusText || activeStepDetail;

	return (
		<Card className="p-4 md:p-5">
			<div className="flex flex-col gap-4">
				<div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
					<div className="min-w-0 space-y-2">
						<div className="flex flex-wrap items-center gap-2">
							<h2 className="truncate text-lg font-semibold">
								{task.display_name}
							</h2>
							<Badge variant={statusBadgeVariant(task.status)}>
								{formatTaskStatus(t, task.status)}
							</Badge>
							<Badge variant="outline">{formatTaskKind(t, task.kind)}</Badge>
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
									onOpenTargetFolder(parsedResult.target_folder_id ?? null)
								}
							>
								<Icon name="FolderOpen" className="mr-1 size-4" />
								{t("tasks:open_target_folder")}
							</Button>
						) : null}
						{hasExpandableDetails ? (
							<Button
								variant="outline"
								size="sm"
								aria-controls={detailsSectionId}
								aria-expanded={detailsExpanded}
								onClick={() => onToggleDetails(task.id)}
							>
								<Icon
									name={detailsExpanded ? "CaretUp" : "CaretDown"}
									className="mr-1 size-4"
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
								onClick={() => onRetry(task.id)}
								disabled={retrying}
							>
								<Icon
									name={retrying ? "Spinner" : "ArrowCounterClockwise"}
									className={`mr-1 size-4 ${retrying ? "animate-spin" : ""}`}
								/>
								{t("tasks:retry_task")}
							</Button>
						) : null}
					</div>
				</div>

				<TaskStepsPreview task={task} />

				{taskSummaryText ? (
					<div className="space-y-2">
						<p className="text-sm text-muted-foreground">
							{t("tasks:status_text_label")}: {taskSummaryText}
						</p>
					</div>
				) : null}

				<AnimatedTaskDetails open={detailsExpanded} className="space-y-2.5">
					<div id={detailsSectionId} className="space-y-2.5 pt-0.5">
						<TaskDetailsContent task={task} />
					</div>
				</AnimatedTaskDetails>
			</div>
		</Card>
	);
}
