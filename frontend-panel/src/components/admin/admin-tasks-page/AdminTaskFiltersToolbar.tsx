import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";

interface AdminTaskFiltersToolbarProps {
	activeFilterCount: number;
	hasServerFilters: boolean;
	kindFilter: string;
	kindOptions: ReadonlyArray<{ label: string; value: string }>;
	onKindChange: (value: string | null) => void;
	onResetFilters: () => void;
	onStatusChange: (value: string | null) => void;
	statusFilter: string;
	statusOptions: ReadonlyArray<{ label: string; value: string }>;
}

export function AdminTaskFiltersToolbar({
	activeFilterCount,
	hasServerFilters,
	kindFilter,
	kindOptions,
	onKindChange,
	onResetFilters,
	onStatusChange,
	statusFilter,
	statusOptions,
}: AdminTaskFiltersToolbarProps) {
	const { t } = useTranslation("admin");

	return (
		<>
			<Select
				items={kindOptions}
				value={kindFilter}
				onValueChange={onKindChange}
			>
				<SelectTrigger width="compact">
					<SelectValue />
				</SelectTrigger>
				<SelectContent>
					{kindOptions.map((option) => (
						<SelectItem key={option.value} value={option.value}>
							{option.label}
						</SelectItem>
					))}
				</SelectContent>
			</Select>
			<Select
				items={statusOptions}
				value={statusFilter}
				onValueChange={onStatusChange}
			>
				<SelectTrigger width="compact">
					<SelectValue />
				</SelectTrigger>
				<SelectContent>
					{statusOptions.map((option) => (
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
