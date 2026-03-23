import { useTranslation } from "react-i18next";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";

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
	const { t } = useTranslation(["common", "files"]);

	return (
		<ConfirmDialog
			open={open}
			onOpenChange={onOpenChange}
			title={t("common:are_you_sure")}
			description={t("files:unsaved_confirm_desc")}
			confirmLabel={t("files:discard_changes")}
			onConfirm={onConfirm}
			variant="destructive"
		/>
	);
}
