import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

interface AdminSurfaceProps {
	children: ReactNode;
	className?: string;
}

export function AdminSurface({ children, className }: AdminSurfaceProps) {
	return (
		<div
			className={cn(
				"min-h-0 flex-1 rounded-xl border bg-background px-3 md:px-4",
				className,
			)}
		>
			{children}
		</div>
	);
}
