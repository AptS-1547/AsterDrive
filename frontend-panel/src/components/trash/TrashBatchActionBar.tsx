import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";

interface TrashBatchActionBarProps {
	count: number;
	pendingOperation?: "restore" | "purge" | "purge-all" | null;
	onRestore: () => void;
	onPurge: () => void;
	onClearSelection: () => void;
}

export function TrashBatchActionBar({
	count,
	pendingOperation = null,
	onRestore,
	onPurge,
	onClearSelection,
}: TrashBatchActionBarProps) {
	const { t } = useTranslation(["core", "files"]);
	const busy = pendingOperation !== null;
	const restoring = pendingOperation === "restore";
	const purging = pendingOperation === "purge";

	if (count === 0) return null;

	return (
		<div className="fixed bottom-4 left-1/2 z-(--z-fixed) flex -translate-x-1/2 items-center gap-2 rounded-xl border border-border/70 bg-card/95 px-4 py-2 shadow-lg shadow-black/8 backdrop-blur supports-[backdrop-filter]:bg-card/85 dark:shadow-none">
			<span className="text-sm font-medium">
				{t("selected_count", { count })}
			</span>
			<div className="flex items-center gap-1">
				<Button size="sm" variant="outline" onClick={onRestore} disabled={busy}>
					<Icon
						name={restoring ? "Spinner" : "ArrowCounterClockwise"}
						className={`mr-1 size-3.5 ${restoring ? "animate-spin" : ""}`}
					/>
					{restoring
						? t("files:trash_restoring")
						: t("files:trash_restore_selected")}
				</Button>
				<Button
					size="sm"
					variant="destructive"
					onClick={onPurge}
					disabled={busy}
				>
					<Icon
						name={purging ? "Spinner" : "Trash"}
						className={`mr-1 size-3.5 ${purging ? "animate-spin" : ""}`}
					/>
					{purging
						? t("files:trash_purging")
						: t("files:trash_delete_selected")}
				</Button>
			</div>
			<Button
				size="sm"
				variant="ghost"
				onClick={onClearSelection}
				disabled={busy}
			>
				<Icon name="X" className="size-3.5" />
			</Button>
		</div>
	);
}
