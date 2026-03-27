import type { ReactNode } from "react";

interface ToolbarBarProps {
	left: ReactNode;
	right?: ReactNode;
}

export function ToolbarBar({ left, right }: ToolbarBarProps) {
	return (
		<div className="border-b border-border/70 bg-background px-4 py-2.5">
			<div className="flex items-center justify-between gap-3">
				<div className="flex h-10 min-w-0 flex-1 items-center gap-2 rounded-lg border border-border/70 bg-muted/20 px-3">
					{left}
				</div>
				{right && (
					<div className="flex h-10 items-center gap-2 rounded-lg border border-border/70 bg-muted/20 px-2">
						{right}
					</div>
				)}
			</div>
		</div>
	);
}
