import type { ReactNode } from "react";
import { useCallback, useState } from "react";
import { Sidebar } from "@/components/layout/Sidebar";
import { TopBar } from "@/components/layout/TopBar";

interface AppLayoutProps {
	children: ReactNode;
	actions?: ReactNode;
}

export function AppLayout({ children, actions }: AppLayoutProps) {
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
				<Sidebar mobileOpen={mobileOpen} onMobileClose={handleMobileClose} />
				<main className="flex-1 flex flex-col overflow-hidden">{children}</main>
			</div>
		</div>
	);
}
