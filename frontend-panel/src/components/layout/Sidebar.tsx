import { useTranslation } from "react-i18next";
import { Link, useLocation } from "react-router-dom";
import { FolderTree } from "@/components/folders/FolderTree";
import { Icon, type IconName } from "@/components/ui/icon";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { formatBytes } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useAuthStore } from "@/stores/authStore";

interface SidebarProps {
	mobileOpen: boolean;
	onMobileClose: () => void;
}

export function Sidebar({ mobileOpen, onMobileClose }: SidebarProps) {
	const { t } = useTranslation();
	const location = useLocation();
	const user = useAuthStore((s) => s.user);

	const navLinks: { to: string; icon: IconName; label: string }[] = [
		{ to: "/trash", icon: "Trash", label: t("trash") },
		{ to: "/settings/webdav", icon: "HardDrive", label: t("webdav") },
		{ to: "/settings", icon: "Gear", label: t("settings") },
	];

	const sidebarContent = (
		<div className="flex flex-col h-full">
			{/* Folder tree */}
			<ScrollArea className="flex-1">
				<FolderTree />
			</ScrollArea>

			<Separator />

			{/* Navigation links */}
			<div className="p-2 space-y-1">
				{navLinks.map((link) => (
					<Link
						key={link.to}
						to={link.to}
						onClick={onMobileClose}
						className={cn(
							"flex items-center gap-2 px-3 py-2 text-sm rounded-md transition-colors",
							location.pathname === link.to
								? "bg-accent text-accent-foreground"
								: "text-muted-foreground hover:text-foreground hover:bg-accent/50",
						)}
					>
						<Icon name={link.icon} className="h-4 w-4 shrink-0" />
						{link.label}
					</Link>
				))}
			</div>

			{/* Storage usage */}
			{user && (
				<>
					<Separator />
					<div className="p-3 space-y-1.5">
						<p className="text-xs font-medium text-muted-foreground">
							{t("files:storage_space")}
						</p>
						<Progress
							value={
								user.storage_quota > 0
									? Math.min(
											(user.storage_used / user.storage_quota) * 100,
											100,
										)
									: 0
							}
							className="h-1.5"
						/>
						<p className="text-xs text-muted-foreground">
							{user.storage_quota > 0
								? t("files:storage_quota", {
										used: formatBytes(user.storage_used),
										quota: formatBytes(user.storage_quota),
									})
								: t("files:storage_used", {
										used: formatBytes(user.storage_used),
									})}
						</p>
					</div>
				</>
			)}
		</div>
	);

	return (
		<>
			{/* Mobile overlay backdrop */}
			{mobileOpen && (
				<button
					type="button"
					className="fixed inset-0 bg-black/50 z-40 md:hidden cursor-default"
					onClick={onMobileClose}
					aria-label="Close sidebar"
				/>
			)}

			{/* Sidebar - desktop inline, mobile overlay */}
			<aside
				className={cn(
					"border-r flex flex-col bg-background transition-all duration-200",
					// Desktop
					"hidden md:flex",
					"w-60",
					// Mobile override when open
					mobileOpen &&
						"fixed inset-y-0 left-0 z-50 flex w-60 md:relative md:z-auto",
				)}
			>
				{sidebarContent}
			</aside>
		</>
	);
}
