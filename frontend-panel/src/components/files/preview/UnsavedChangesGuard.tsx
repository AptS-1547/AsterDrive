import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";

interface UnsavedChangesGuardProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onConfirm: () => void;
}

export function UnsavedChangesGuard({
	open,
	onOpenChange,
	onConfirm,
}: UnsavedChangesGuardProps) {
	const { t } = useTranslation(["core", "files"]);

	if (!open) {
		return null;
	}

	return (
		<div className="fixed inset-x-3 bottom-3 z-(--z-alert-dialog) mx-auto flex max-w-xl flex-col gap-3 rounded-xl border border-destructive/30 bg-popover p-4 text-sm shadow-2xl shadow-black/15 ring-1 ring-foreground/5 sm:bottom-5 sm:flex-row sm:items-center sm:justify-between dark:shadow-none">
			<div>
				<p className="font-medium text-foreground">{t("are_you_sure")}</p>
				<p className="mt-1 text-muted-foreground">
					{t("files:unsaved_confirm_desc")}
				</p>
			</div>
			<div className="flex shrink-0 items-center justify-end gap-2">
				<Button
					type="button"
					variant="outline"
					onClick={() => onOpenChange(false)}
				>
					{t("cancel")}
				</Button>
				<Button type="button" variant="destructive" onClick={onConfirm}>
					{t("files:discard_changes")}
				</Button>
			</div>
		</div>
	);
}
