import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { LanguageSwitcher } from "@/components/common/LanguageSwitcher";
import { ThemeSwitcher } from "@/components/common/ThemeSwitcher";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { useAuthStore } from "@/stores/authStore";
import { useFileStore } from "@/stores/fileStore";

interface TopBarProps {
	onSidebarToggle: () => void;
	actions?: ReactNode;
}

export function TopBar({ onSidebarToggle, actions }: TopBarProps) {
	const { t } = useTranslation();
	const navigate = useNavigate();
	const user = useAuthStore((s) => s.user);
	const isAuthStale = useAuthStore((s) => s.isAuthStale);
	const logout = useAuthStore((s) => s.logout);
	const search = useFileStore((s) => s.search);
	const clearSearch = useFileStore((s) => s.clearSearch);
	const activeQuery = useFileStore((s) => s.searchQuery);
	const [searchInput, setSearchInput] = useState("");

	// Clear input when search is cleared (e.g., by navigating to a folder)
	useEffect(() => {
		if (activeQuery === null) setSearchInput("");
	}, [activeQuery]);

	const handleSearch = (e: React.KeyboardEvent) => {
		if (e.key === "Enter" && searchInput.trim()) {
			// Navigate to file browser if not already there
			if (window.location.pathname !== "/") navigate("/");
			search(searchInput.trim());
		}
		if (e.key === "Escape" && activeQuery) {
			setSearchInput("");
			clearSearch();
		}
	};

	return (
		<div className="h-14 border-b flex items-center px-4 gap-3 shrink-0">
			{/* Left: hamburger + title */}
			<Button
				variant="ghost"
				size="icon"
				className="h-8 w-8 md:hidden shrink-0"
				onClick={onSidebarToggle}
			>
				<Icon name="List" className="h-4 w-4" />
			</Button>

			<img
				src="/static/logo.svg"
				alt={t("app_name")}
				className="hidden h-10 w-auto shrink-0 md:block"
			/>

			{/* Center: search */}
			<div className="flex-1 max-w-md hidden sm:flex items-center">
				<div className="relative w-full">
					<Icon
						name="MagnifyingGlass"
						className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground"
					/>
					<Input
						value={searchInput}
						onChange={(e) => setSearchInput(e.target.value)}
						onKeyDown={handleSearch}
						placeholder={t("search_placeholder")}
						className="h-8 pl-8 pr-8 text-sm bg-muted/50 border-transparent focus-visible:border-border"
					/>
					{activeQuery && (
						<button
							type="button"
							title={t("clear_search")}
							aria-label={t("clear_search")}
							className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
							onClick={() => {
								setSearchInput("");
								clearSearch();
							}}
						>
							<Icon name="X" className="h-3.5 w-3.5" />
						</button>
					)}
				</div>
			</div>

			{/* Spacer to push right section */}
			<div className="flex-1" />

			{/* Right: page actions + theme + lang + user */}
			<div className="flex items-center gap-1 shrink-0">
				{actions}
				{isAuthStale && (
					<Tooltip>
						<TooltipTrigger
							render={
								<Badge
									variant="outline"
									className="hidden cursor-help items-center gap-1.5 border-amber-500/40 bg-amber-500/10 text-amber-700 md:inline-flex dark:text-amber-300"
								/>
							}
						>
							<Icon name="Warning" className="h-3.5 w-3.5" />
							<span>{t("offline_status_short")}</span>
						</TooltipTrigger>
						<TooltipContent className="max-w-64 text-left leading-relaxed">
							<div>{t("offline_mode")}</div>
							<div className="text-background/80">{t("auth_stale_detail")}</div>
						</TooltipContent>
					</Tooltip>
				)}
				<ThemeSwitcher />
				<LanguageSwitcher />

				{/* User dropdown */}
				<DropdownMenu>
					<DropdownMenuTrigger
						render={<Button variant="ghost" size="sm" className="gap-1.5" />}
					>
						<span className="text-sm truncate max-w-24">{user?.username}</span>
						{user?.role === "admin" && (
							<Badge
								variant="secondary"
								className="text-xs px-1.5 py-0 hidden sm:inline-flex"
							>
								admin
							</Badge>
						)}
					</DropdownMenuTrigger>
					<DropdownMenuContent align="end">
						{user?.role === "admin" && (
							<>
								<DropdownMenuItem onClick={() => navigate("/admin")}>
									<Icon name="Shield" className="h-4 w-4 mr-2" />
									{t("admin_panel")}
								</DropdownMenuItem>
								<DropdownMenuSeparator />
							</>
						)}
						<DropdownMenuItem onClick={logout}>
							<Icon name="SignOut" className="h-4 w-4 mr-2" />
							{t("logout")}
						</DropdownMenuItem>
					</DropdownMenuContent>
				</DropdownMenu>
			</div>
		</div>
	);
}
