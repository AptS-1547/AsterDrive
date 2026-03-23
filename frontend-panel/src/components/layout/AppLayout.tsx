import type { ReactNode } from "react";
import { useCallback, useState } from "react";
import { Sidebar } from "@/components/layout/Sidebar";
import { TopBar } from "@/components/layout/TopBar";
import { STORAGE_KEYS } from "@/config/app";

interface AppLayoutProps {
	children: ReactNode;
	title?: string;
	actions?: ReactNode;
}

function getInitialCollapsed(): boolean {
	if (typeof window === "undefined") return false;
	return localStorage.getItem(STORAGE_KEYS.sidebarCollapsed) === "true";
}

export function AppLayout({ children, title, actions }: AppLayoutProps) {
	const [collapsed, setCollapsed] = useState(getInitialCollapsed);
	const [mobileOpen, setMobileOpen] = useState(false);

	const handleToggle = useCallback(() => {
		setCollapsed((prev) => {
			const next = !prev;
			localStorage.setItem(STORAGE_KEYS.sidebarCollapsed, String(next));
			return next;
		});
	}, []);

	const handleMobileToggle = useCallback(() => {
		setMobileOpen((prev) => !prev);
	}, []);

	const handleMobileClose = useCallback(() => {
		setMobileOpen(false);
	}, []);

	return (
		<div className="h-screen flex flex-col animate-in fade-in duration-300">
			<TopBar
				onSidebarToggle={handleMobileToggle}
				title={title}
				actions={actions}
			/>
			<div className="flex flex-1 overflow-hidden">
				<Sidebar
					collapsed={collapsed}
					onToggle={handleToggle}
					mobileOpen={mobileOpen}
					onMobileClose={handleMobileClose}
				/>
				<main className="flex-1 flex flex-col overflow-hidden">{children}</main>
			</div>
		</div>
	);
}
