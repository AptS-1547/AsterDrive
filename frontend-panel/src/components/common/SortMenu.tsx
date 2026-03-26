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

	return (
		<DropdownMenu>
			<DropdownMenuTrigger
				render={
					<Button variant="ghost" size="sm" className="h-8 gap-1.5 px-2">
						<Icon
							name={sortOrder === "asc" ? "SortAscending" : "SortDescending"}
							className="h-4 w-4"
						/>
						<span className="text-xs">{t(`sort_${sortBy}`)}</span>
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
						<Icon name="SortAscending" className="mr-2 h-4 w-4" />
						{t("sort_asc")}
					</DropdownMenuRadioItem>
					<DropdownMenuRadioItem value="desc">
						<Icon name="SortDescending" className="mr-2 h-4 w-4" />
						{t("sort_desc")}
					</DropdownMenuRadioItem>
				</DropdownMenuRadioGroup>
			</DropdownMenuContent>
		</DropdownMenu>
	);
}
