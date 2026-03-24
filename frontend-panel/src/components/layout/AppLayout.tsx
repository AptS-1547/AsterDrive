import type { ReactNode } from "react";
import { useCallback, useState } from "react";
import { Sidebar } from "@/components/layout/Sidebar";
import { TopBar } from "@/components/layout/TopBar";
import type { InternalDragData } from "@/lib/dragDrop";

interface AppLayoutProps {
	children: ReactNode;
	actions?: ReactNode;
	onTrashDrop?: (data: InternalDragData) => void | Promise<void>;
}

export function AppLayout({ children, actions, onTrashDrop }: AppLayoutProps) {
	const [mobileOpen, setMobileOpen] = useState(false);

	const handleMobileToggle = useCallback(() => {
		setMobileOpen((prev) => !prev);
	}, []);

	const handleMobileClose = useCallback(() => {
		setMobileOpen(false);
	}, []);

	return (
		<div className="h-screen flex flex-col">
			<TopBar onSidebarToggle={handleMobileToggle} actions={actions} />
			<div className="flex flex-1 overflow-hidden">
				<Sidebar
					mobileOpen={mobileOpen}
					onMobileClose={handleMobileClose}
					onTrashDrop={onTrashDrop}
				/>
				<main className="min-h-0 min-w-0 flex-1 flex flex-col overflow-hidden">
					{children}
				</main>
			</div>
		</div>
	);
}
