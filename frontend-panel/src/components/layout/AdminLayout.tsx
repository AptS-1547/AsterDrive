import {
	ArrowLeft,
	HardDrive,
	Link,
	Lock,
	Settings,
	Users,
} from "lucide-react";
import type { ReactNode } from "react";
import { NavLink } from "react-router-dom";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";

const navItems = [
	{ to: "/admin/users", label: "Users", icon: Users },
	{ to: "/admin/policies", label: "Storage Policies", icon: HardDrive },
	{ to: "/admin/shares", label: "Shares", icon: Link },
	{ to: "/admin/locks", label: "Locks", icon: Lock },
	{ to: "/admin/settings", label: "Settings", icon: Settings },
];

interface AdminLayoutProps {
	children: ReactNode;
}

export function AdminLayout({ children }: AdminLayoutProps) {
	return (
		<div className="h-screen flex flex-col">
			<div className="flex flex-1 overflow-hidden">
				<aside className="w-56 border-r flex flex-col">
					<div className="p-3 flex items-center gap-2">
						<NavLink
							to="/"
							className="text-muted-foreground hover:text-foreground transition-colors"
						>
							<ArrowLeft className="h-4 w-4" />
						</NavLink>
						<span className="font-semibold text-sm uppercase tracking-wider text-muted-foreground">
							Admin
						</span>
					</div>
					<Separator />
					<ScrollArea className="flex-1">
						<nav className="p-2 space-y-1">
							{navItems.map((item) => (
								<NavLink
									key={item.to}
									to={item.to}
									className={({ isActive }) =>
										`flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
											isActive
												? "bg-accent text-accent-foreground font-medium"
												: "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
										}`
									}
								>
									<item.icon className="h-4 w-4" />
									{item.label}
								</NavLink>
							))}
						</nav>
					</ScrollArea>
				</aside>
				<main className="flex-1 flex flex-col overflow-hidden">{children}</main>
			</div>
		</div>
	);
}
