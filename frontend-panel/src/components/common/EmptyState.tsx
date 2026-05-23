import type { ReactNode } from "react";

interface EmptyStateProps {
	icon?: ReactNode;
	title: string;
	description?: string;
	action?: ReactNode;
}

export function EmptyState({
	icon,
	title,
	description,
	action,
}: EmptyStateProps) {
	return (
		<div className="flex flex-col items-center justify-center px-4 py-16 text-center">
			{icon && (
				<div className="mb-4 flex size-14 items-center justify-center rounded-xl border border-border/60 bg-card text-muted-foreground shadow-xs dark:bg-muted/25 dark:shadow-none [&_svg]:h-7 [&_svg]:w-7">
					{icon}
				</div>
			)}
			<h3 className="text-base font-semibold">{title}</h3>
			{description && (
				<p className="mt-1.5 max-w-sm text-sm text-muted-foreground">
					{description}
				</p>
			)}
			{action && <div className="mt-4">{action}</div>}
		</div>
	);
}
