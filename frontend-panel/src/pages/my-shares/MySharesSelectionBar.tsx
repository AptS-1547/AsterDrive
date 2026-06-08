import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import type { MyShareInfo } from "@/types/api";

interface MySharesSelectionBarProps {
	batchDeleteLabel: string;
	editLabel: string;
	onClear: () => void;
	onDelete: (shares: MyShareInfo[]) => void;
	onEdit: (share: MyShareInfo) => void;
	selectedCountLabel: string;
	selectedShares: MyShareInfo[];
}

export function MySharesSelectionBar({
	batchDeleteLabel,
	editLabel,
	onClear,
	onDelete,
	onEdit,
	selectedCountLabel,
	selectedShares,
}: MySharesSelectionBarProps) {
	if (selectedShares.length === 0) {
		return null;
	}

	return (
		<div className="fixed bottom-4 left-1/2 z-(--z-fixed) flex -translate-x-1/2 items-center gap-2 rounded-xl border border-border/70 bg-card/95 px-4 py-2 shadow-lg shadow-black/8 backdrop-blur supports-[backdrop-filter]:bg-card/85 dark:shadow-none">
			<span className="text-sm font-medium">{selectedCountLabel}</span>
			<div className="flex items-center gap-1">
				{selectedShares.length === 1 ? (
					<Button
						size="sm"
						variant="outline"
						onClick={() => onEdit(selectedShares[0])}
					>
						<Icon name="PencilSimple" className="mr-1 size-3.5" />
						{editLabel}
					</Button>
				) : null}
				<Button
					size="sm"
					variant="destructive"
					onClick={() => onDelete(selectedShares)}
				>
					<Icon name="Trash" className="mr-1 size-3.5" />
					{batchDeleteLabel}
				</Button>
			</div>
			<Button size="sm" variant="ghost" onClick={onClear}>
				<Icon name="X" className="size-3.5" />
			</Button>
		</div>
	);
}
