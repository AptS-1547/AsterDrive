import { useTranslation } from "react-i18next";
import { UploadTaskItem } from "@/components/files/UploadTaskItem";
import { Button } from "@/components/ui/button";
import {
	Card,
	CardContent,
	CardFooter,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import { Icon } from "@/components/ui/icon";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";

export interface UploadTaskView {
	id: string;
	title: string;
	status: string;
	mode: string;
	progress: number;
	group?: string;
	targetLabel?: string;
	batchStatus?: "active" | "done" | "partial_failed";
	detail?: string;
	completed?: boolean;
	actions?: {
		label: string;
		icon: "X" | "ArrowsClockwise";
		onClick: () => void;
		variant?: "outline" | "ghost";
	}[];
}

interface UploadPanelProps {
	open: boolean;
	onToggle: () => void;
	title: string;
	summary: string;
	tasks: UploadTaskView[];
	emptyText: string;
	totalCount?: number;
	successCount?: number;
	failedCount?: number;
	activeCount?: number;
	overallProgress?: number;
	onRetryFailed?: () => void;
	retryFailedLabel?: string;
	onClearCompleted?: () => void;
	clearCompletedLabel?: string;
}

export function UploadPanel({
	open,
	onToggle,
	title,
	summary,
	tasks,
	emptyText,
	totalCount = 0,
	successCount = 0,
	failedCount = 0,
	activeCount = 0,
	overallProgress = 0,
	onRetryFailed,
	retryFailedLabel,
	onClearCompleted,
	clearCompletedLabel,
}: UploadPanelProps) {
	const { t } = useTranslation("files");
	const groupedTasks = tasks.reduce<Record<string, UploadTaskView[]>>(
		(acc, task) => {
			const key = task.group ?? "";
			acc[key] ??= [];
			acc[key].push(task);
			return acc;
		},
		{},
	);

	const groupedEntries = Object.entries(groupedTasks).map(
		([group, groupTasks]) => {
			const success = groupTasks.filter((task) => task.completed).length;
			const failed = groupTasks.filter((task) =>
				task.actions?.some((action) => action.icon === "ArrowsClockwise"),
			).length;
			const active = groupTasks.filter(
				(task) =>
					!task.completed &&
					!task.actions?.some((action) => action.icon === "ArrowsClockwise"),
			).length;
			return {
				group,
				groupTasks,
				success,
				failed,
				active,
				total: groupTasks.length,
				targetLabel: groupTasks[0]?.targetLabel,
				batchStatus:
					active > 0 ? "active" : failed > 0 ? "partial_failed" : "done",
			};
		},
	);

	return (
		<div className="absolute right-4 bottom-4 z-40 w-[22rem] max-w-[calc(100vw-2rem)]">
			<Card
				size="sm"
				className={`flex flex-col overflow-hidden shadow-xl backdrop-blur-sm ${
					open ? "h-[min(32rem,calc(100vh-6rem))]" : ""
				}`}
			>
				<CardHeader className="border-b">
					<div className="flex items-center gap-2">
						<Icon name="Upload" className="h-4 w-4 text-muted-foreground" />
						<div className="min-w-0 flex-1">
							<CardTitle>{title}</CardTitle>
							<div className="truncate text-xs text-muted-foreground">
								{summary}
							</div>
						</div>
						<Button variant="ghost" size="icon-xs" onClick={onToggle}>
							<Icon name={open ? "CaretDown" : "CaretUp"} className="h-3 w-3" />
						</Button>
					</div>
				</CardHeader>
				{open && (
					<>
						<div className="space-y-3 border-b px-4 py-3">
							<div className="grid grid-cols-4 gap-2 text-center text-xs">
								<div className="rounded-md bg-muted/50 px-2 py-1">
									<div className="text-[10px] text-muted-foreground">
										{t("upload_stat_total")}
									</div>
									<div className="font-medium">{totalCount}</div>
								</div>
								<div className="rounded-md bg-emerald-500/10 px-2 py-1">
									<div className="text-[10px] text-muted-foreground">
										{t("upload_stat_success")}
									</div>
									<div className="font-medium text-emerald-600">
										{successCount}
									</div>
								</div>
								<div className="rounded-md bg-rose-500/10 px-2 py-1">
									<div className="text-[10px] text-muted-foreground">
										{t("upload_stat_failed")}
									</div>
									<div className="font-medium text-rose-600">{failedCount}</div>
								</div>
								<div className="rounded-md bg-primary/10 px-2 py-1">
									<div className="text-[10px] text-muted-foreground">
										{t("upload_stat_active")}
									</div>
									<div className="font-medium text-primary">{activeCount}</div>
								</div>
							</div>
							<div className="space-y-1">
								<div className="flex items-center justify-between text-[11px] text-muted-foreground">
									<span>{t("upload_overall_progress")}</span>
									<span>{overallProgress}%</span>
								</div>
								<Progress value={overallProgress} className="h-2" />
							</div>
						</div>
						<CardContent className="min-h-0 flex-1 overflow-hidden p-0">
							<ScrollArea className="h-full">
								<div className="space-y-2 p-3">
									{tasks.length === 0 ? (
										<div className="py-8 text-center text-sm text-muted-foreground">
											{emptyText}
										</div>
									) : (
										groupedEntries.map(
											({
												group,
												groupTasks,
												success,
												failed,
												active,
												total,
												targetLabel,
												batchStatus,
											}) => (
												<div key={group || "root"} className="space-y-2">
													<div className="space-y-1 px-1 text-[11px] text-muted-foreground">
														<div className="flex items-center justify-between gap-2">
															<div className="min-w-0 truncate font-medium">
																{group || t("root")}
															</div>
															<div className="shrink-0">
																{batchStatus === "active"
																	? t("upload_batch_active")
																	: batchStatus === "partial_failed"
																		? t("upload_batch_partial_failed")
																		: t("upload_batch_done")}
															</div>
														</div>
														<div className="flex items-center justify-between gap-2">
															<div className="min-w-0 truncate">
																{t("upload_target_location")}：
																{targetLabel ?? t("upload_target_current")}
															</div>
															<div className="shrink-0">
																{t("upload_group_stats", {
																	total,
																	success,
																	failed,
																	active,
																})}
															</div>
														</div>
													</div>
													{groupTasks.map((task) => (
														<UploadTaskItem key={task.id} {...task} />
													))}
												</div>
											),
										)
									)}
								</div>
							</ScrollArea>
						</CardContent>
						<CardFooter className="shrink-0 justify-end gap-2 border-t">
							{onRetryFailed && retryFailedLabel && failedCount > 0 && (
								<Button variant="outline" size="sm" onClick={onRetryFailed}>
									{retryFailedLabel}
								</Button>
							)}
							{onClearCompleted && clearCompletedLabel && (
								<Button variant="outline" size="sm" onClick={onClearCompleted}>
									{clearCompletedLabel}
								</Button>
							)}
						</CardFooter>
					</>
				)}
			</Card>
		</div>
	);
}
