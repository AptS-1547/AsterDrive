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
import { ScrollArea } from "@/components/ui/scroll-area";

export interface UploadTaskView {
	id: string;
	title: string;
	status: string;
	mode: string;
	progress: number;
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
	onClearCompleted,
	clearCompletedLabel,
}: UploadPanelProps) {
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
						<CardContent className="min-h-0 flex-1 overflow-hidden p-0">
							<ScrollArea className="h-full">
								<div className="space-y-2 p-3">
									{tasks.length === 0 ? (
										<div className="py-8 text-center text-sm text-muted-foreground">
											{emptyText}
										</div>
									) : (
										tasks.map((task) => (
											<UploadTaskItem key={task.id} {...task} />
										))
									)}
								</div>
							</ScrollArea>
						</CardContent>
						<CardFooter className="shrink-0 justify-end gap-2 border-t">
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
