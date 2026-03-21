import type { ReactNode } from "react";
import { Link, useLocation } from "react-router-dom";
import { HardDrive, Trash2 } from "lucide-react";
import { FolderTree } from "@/components/folders/FolderTree";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";

interface AppLayoutProps {
	children: ReactNode;
}

export function AppLayout({ children }: AppLayoutProps) {
	const location = useLocation();

	return (
		<div className="h-screen flex flex-col">
			<div className="flex flex-1 overflow-hidden">
				{/* Left sidebar */}
				<aside className="w-60 border-r flex flex-col">
					<div className="p-3 font-semibold text-sm text-muted-foreground uppercase tracking-wider">
						Folders
					</div>
					<Separator />
					<ScrollArea className="flex-1">
						<FolderTree />
					</ScrollArea>
					<Separator />
					<div className="p-2 space-y-1">
						<Link
							to="/trash"
							className={`flex items-center gap-2 px-3 py-2 text-sm rounded-md transition-colors ${
								location.pathname === "/trash"
									? "bg-accent text-accent-foreground"
									: "text-muted-foreground hover:text-foreground hover:bg-accent/50"
							}`}
						>
							<Trash2 className="h-4 w-4" />
							Trash
						</Link>
						<Link
							to="/settings/webdav"
							className={`flex items-center gap-2 px-3 py-2 text-sm rounded-md transition-colors ${
								location.pathname === "/settings/webdav"
									? "bg-accent text-accent-foreground"
									: "text-muted-foreground hover:text-foreground hover:bg-accent/50"
							}`}
						>
							<HardDrive className="h-4 w-4" />
							WebDAV
						</Link>
					</div>
				</aside>

				{/* Right content */}
				<main className="flex-1 flex flex-col overflow-hidden">{children}</main>
			</div>
		</div>
	);
}
