import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";

interface AdminTeamsToolbarProps {
	activeFilterCount: number;
	hasServerFilters: boolean;
	keyword: string;
	onArchivedToggle: () => void;
	onKeywordChange: (value: string) => void;
	onResetFilters: () => void;
	showArchived: boolean;
}

export function AdminTeamsToolbar({
	activeFilterCount,
	hasServerFilters,
	keyword,
	onArchivedToggle,
	onKeywordChange,
	onResetFilters,
	showArchived,
}: AdminTeamsToolbarProps) {
	const { t } = useTranslation("admin");

	return (
		<>
			<div className="relative min-w-[240px] flex-1 md:max-w-sm">
				<Icon
					name="MagnifyingGlass"
					className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
				/>
				<Input
					value={keyword}
					onChange={(event) => onKeywordChange(event.target.value)}
					placeholder={t("team_search_placeholder")}
					className={`${ADMIN_CONTROL_HEIGHT_CLASS} pl-9`}
				/>
			</div>
			<Button
				variant={showArchived ? "default" : "outline"}
				size="sm"
				className={ADMIN_CONTROL_HEIGHT_CLASS}
				onClick={onArchivedToggle}
			>
				<Icon name="Cloud" className="mr-1 size-4" />
				{showArchived ? t("show_active_teams") : t("show_archived_teams")}
			</Button>
			<div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
				{hasServerFilters ? <span>{t("filters_active")}</span> : null}
				{activeFilterCount > 0 ? (
					<Button
						variant="ghost"
						size="sm"
						className={ADMIN_CONTROL_HEIGHT_CLASS}
						onClick={onResetFilters}
					>
						{t("clear_filters")}
					</Button>
				) : null}
			</div>
		</>
	);
}
