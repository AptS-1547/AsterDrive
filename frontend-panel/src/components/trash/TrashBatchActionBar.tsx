import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";

interface TrashBatchActionBarProps {
	count: number;
	onRestore: () => void;
	onPurge: () => void;
	onClearSelection: () => void;
}

export function TrashBatchActionBar({
	count,
	onRestore,
	onPurge,
	onClearSelection,
}: TrashBatchActionBarProps) {
	const { t } = useTranslation(["common", "files"]);

	if (count === 0) return null;

	return (
		<div className="fixed bottom-4 left-1/2 z-50 flex -translate-x-1/2 items-center gap-2 rounded-lg border bg-background px-4 py-2 shadow-lg">
			<span className="text-sm font-medium">
				{t("common:selected_count", { count })}
			</span>
			<div className="flex items-center gap-1">
				<Button size="sm" variant="outline" onClick={onRestore}>
					<Icon name="ArrowCounterClockwise" className="mr-1 h-3.5 w-3.5" />
					{t("files:trash_restore_selected")}
				</Button>
				<Button size="sm" variant="destructive" onClick={onPurge}>
					<Icon name="Trash" className="mr-1 h-3.5 w-3.5" />
					{t("files:trash_delete_selected")}
				</Button>
			</div>
			<Button size="sm" variant="ghost" onClick={onClearSelection}>
				<Icon name="X" className="h-3.5 w-3.5" />
			</Button>
		</div>
	);
}
