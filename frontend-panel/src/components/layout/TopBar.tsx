import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useLocation, useNavigate } from "react-router-dom";
import { AsterDriveWordmark } from "@/components/common/AsterDriveWordmark";
import { HeaderControls } from "@/components/layout/HeaderControls";
import { TopBarShell } from "@/components/layout/TopBarShell";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { workspaceRootPath } from "@/lib/workspace";
import { useFileStore } from "@/stores/fileStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

interface TopBarProps {
	onSidebarToggle: () => void;
	actions?: ReactNode;
}

export function TopBar({ onSidebarToggle, actions }: TopBarProps) {
	const { t } = useTranslation();
	const navigate = useNavigate();
	const location = useLocation();
	const workspace = useWorkspaceStore((s) => s.workspace);
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
			const rootPath = workspaceRootPath(workspace);
			if (location.pathname !== rootPath) {
				navigate(rootPath);
			}
			search(searchInput.trim());
		}
		if (e.key === "Escape" && activeQuery) {
			setSearchInput("");
			clearSearch();
		}
	};

	return (
		<TopBarShell
			onSidebarToggle={onSidebarToggle}
			left={
				<AsterDriveWordmark
					alt={t("app_name")}
					className="hidden h-16 w-auto shrink-0 md:block px-6"
				/>
			}
			center={
				<div className="flex max-w-md items-center">
					<div className="relative w-full">
						<Icon
							name="MagnifyingGlass"
							className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground"
						/>
						<Input
							value={searchInput}
							onChange={(e) => setSearchInput(e.target.value)}
							onKeyDown={handleSearch}
							placeholder={t("search_placeholder")}
							className="h-8 border-transparent bg-muted/50 pl-8 pr-8 text-sm focus-visible:border-border"
						/>
						{activeQuery ? (
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
						) : null}
					</div>
				</div>
			}
			right={<HeaderControls actions={actions} showAdminEntry />}
		/>
	);
}
