import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { Progress } from "@/components/ui/progress";

interface UploadTaskAction {
	label: string;
	icon: "X" | "ArrowsClockwise";
	onClick: () => void;
	variant?: "outline" | "ghost";
}

interface UploadTaskItemProps {
	title: string;
	status: string;
	mode: string;
	progress: number;
	detail?: string;
	completed?: boolean;
	actions?: UploadTaskAction[];
}

export function UploadTaskItem({
	title,
	status,
	mode,
	progress,
	detail,
	completed = false,
	actions = [],
}: UploadTaskItemProps) {
	return (
		<div
			className={`space-y-2 rounded-lg border p-3 ${
				completed ? "bg-muted/30 opacity-80" : "bg-background/60"
			}`}
		>
			<div className="flex items-start gap-2">
				<div className="min-w-0 flex-1">
					<div className="truncate text-sm font-medium">{title}</div>
				</div>
				{actions.length > 0 && (
					<div className="flex items-center gap-1">
						{actions.map((action) => (
							<Button
								key={`${action.icon}-${action.label}`}
								variant={action.variant ?? "ghost"}
								size="icon-xs"
								onClick={action.onClick}
								title={action.label}
							>
								<Icon name={action.icon} className="h-3 w-3" />
							</Button>
						))}
					</div>
				)}
			</div>
			<Progress value={progress} className="h-1.5" />
			<div className="flex items-center justify-between text-xs text-muted-foreground">
				<div className="flex min-w-0 items-center gap-2">
					<Badge variant="outline" className="font-normal">
						{mode}
					</Badge>
					<span className="truncate">{detail ?? status}</span>
				</div>
				<span className="shrink-0">{progress}%</span>
			</div>
		</div>
	);
}
