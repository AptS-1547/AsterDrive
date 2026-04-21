import type { ReactNode } from "react";
import { PAGE_SECTION_PADDING_CLASS } from "@/lib/constants";
import { cn } from "@/lib/utils";

interface AdminSurfaceProps {
	children: ReactNode;
	className?: string;
	padded?: boolean;
}

export function AdminSurface({
	children,
	className,
	padded = true,
}: AdminSurfaceProps) {
	return (
		<div
			className={cn(
				"flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-xl border bg-background",
				padded && PAGE_SECTION_PADDING_CLASS,
				className,
			)}
		>
			{children}
		</div>
	);
}
