import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";

interface UserFilterOption {
	label: string;
	value: string;
}

interface UsersToolbarProps {
	activeFilterCount: number;
	hasServerFilters: boolean;
	keyword: string;
	roleFilter: string;
	roleFilterOptions: ReadonlyArray<UserFilterOption>;
	statusFilter: string;
	statusFilterOptions: ReadonlyArray<UserFilterOption>;
	onKeywordChange: (value: string) => void;
	onResetFilters: () => void;
	onRoleFilterChange: (value: string | null) => void;
	onStatusFilterChange: (value: string | null) => void;
}

export function UsersToolbar({
	activeFilterCount,
	hasServerFilters,
	keyword,
	onKeywordChange,
	onResetFilters,
	onRoleFilterChange,
	onStatusFilterChange,
	roleFilter,
	roleFilterOptions,
	statusFilter,
	statusFilterOptions,
}: UsersToolbarProps) {
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
					onChange={(e) => onKeywordChange(e.target.value)}
					placeholder={t("user_search_placeholder")}
					className={`${ADMIN_CONTROL_HEIGHT_CLASS} pl-9`}
				/>
			</div>
			<Select
				items={roleFilterOptions}
				value={roleFilter}
				onValueChange={onRoleFilterChange}
			>
				<SelectTrigger width="compact">
					<SelectValue />
				</SelectTrigger>
				<SelectContent>
					{roleFilterOptions.map((option) => (
						<SelectItem key={option.value} value={option.value}>
							{option.label}
						</SelectItem>
					))}
				</SelectContent>
			</Select>
			<Select
				items={statusFilterOptions}
				value={statusFilter}
				onValueChange={onStatusFilterChange}
			>
				<SelectTrigger width="compact">
					<SelectValue />
				</SelectTrigger>
				<SelectContent>
					{statusFilterOptions.map((option) => (
						<SelectItem key={option.value} value={option.value}>
							{option.label}
						</SelectItem>
					))}
				</SelectContent>
			</Select>
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
