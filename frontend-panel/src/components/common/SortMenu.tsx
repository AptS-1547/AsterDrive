import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuRadioGroup,
	DropdownMenuRadioItem,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Icon } from "@/components/ui/icon";
import type { SortBy, SortOrder } from "@/stores/fileStore";

interface SortMenuProps {
	sortBy: SortBy;
	sortOrder: SortOrder;
	onSortBy: (sortBy: SortBy) => void;
	onSortOrder: (sortOrder: SortOrder) => void;
}

const SORT_OPTIONS: SortBy[] = [
	"name",
	"size",
	"created_at",
	"updated_at",
	"type",
];

export function SortMenu({
	sortBy,
	sortOrder,
	onSortBy,
	onSortOrder,
}: SortMenuProps) {
	const { t } = useTranslation("files");
	const triggerLabel = `${t("sort_by")}: ${t(`sort_${sortBy}`)} · ${t(`sort_${sortOrder}`)}`;

	return (
		<DropdownMenu>
			<DropdownMenuTrigger
				render={
					<Button
						aria-label={triggerLabel}
						title={triggerLabel}
						variant="ghost"
						size="sm"
						className="h-7 gap-1 px-1.5 max-[420px]:w-7 max-[420px]:justify-center max-[420px]:px-0 sm:h-8 sm:gap-1.5 sm:px-2"
					>
						<Icon
							name={sortOrder === "asc" ? "SortAscending" : "SortDescending"}
							className="size-4"
						/>
						<span className="max-w-14 truncate text-[11px] max-[420px]:hidden sm:max-w-none sm:text-xs">
							{t(`sort_${sortBy}`)}
						</span>
					</Button>
				}
			/>
			<DropdownMenuContent align="end">
				<DropdownMenuRadioGroup
					value={sortBy}
					onValueChange={(v) => onSortBy(v as SortBy)}
				>
					{SORT_OPTIONS.map((opt) => (
						<DropdownMenuRadioItem key={opt} value={opt}>
							{t(`sort_${opt}`)}
						</DropdownMenuRadioItem>
					))}
				</DropdownMenuRadioGroup>
				<DropdownMenuSeparator />
				<DropdownMenuRadioGroup
					value={sortOrder}
					onValueChange={(v) => onSortOrder(v as SortOrder)}
				>
					<DropdownMenuRadioItem value="asc">
						<Icon name="SortAscending" className="mr-2 size-4" />
						{t("sort_asc")}
					</DropdownMenuRadioItem>
					<DropdownMenuRadioItem value="desc">
						<Icon name="SortDescending" className="mr-2 size-4" />
						{t("sort_desc")}
					</DropdownMenuRadioItem>
				</DropdownMenuRadioGroup>
			</DropdownMenuContent>
		</DropdownMenu>
	);
}
