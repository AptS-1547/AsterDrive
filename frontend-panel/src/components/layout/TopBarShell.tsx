import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { USER_TOPBAR_HEIGHT_CLASS } from "@/lib/constants";

interface TopBarShellProps {
	onSidebarToggle: () => void;
	left: ReactNode;
	center?: ReactNode;
	right: ReactNode;
	heightClassName?: string;
}

export function TopBarShell({
	onSidebarToggle,
	left,
	center,
	right,
	heightClassName = USER_TOPBAR_HEIGHT_CLASS,
}: TopBarShellProps) {
	return (
		<div className="border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80">
			<div
				className={`flex items-center gap-3 px-4 shrink-0 ${heightClassName}`}
			>
				<Button
					variant="ghost"
					size="icon"
					className="h-8 w-8 shrink-0 md:hidden"
					onClick={onSidebarToggle}
				>
					<Icon name="List" className="h-4 w-4" />
				</Button>
				<div className="min-w-0 shrink-0">{left}</div>
				{center ? (
					<div className="hidden min-w-0 flex-1 sm:flex">{center}</div>
				) : (
					<div className="flex-1" />
				)}
				<div className="flex-1" />
				{right}
			</div>
		</div>
	);
}
