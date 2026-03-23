import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { BatchTargetFolderDialog } from "@/components/files/BatchTargetFolderDialog";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { handleApiError } from "@/hooks/useApiError";
import { formatBatchToast } from "@/lib/formatBatchToast";
import { batchService } from "@/services/batchService";
import type { BreadcrumbItem } from "@/stores/fileStore";
import { useFileStore } from "@/stores/fileStore";

export function BatchActionBar() {
	const { t } = useTranslation("files");
	const selectedFileIds = useFileStore((s) => s.selectedFileIds);
	const selectedFolderIds = useFileStore((s) => s.selectedFolderIds);
	const clearSelection = useFileStore((s) => s.clearSelection);
	const refresh = useFileStore((s) => s.refresh);
	const moveToFolder = useFileStore((s) => s.moveToFolder);
	const currentFolderId = useFileStore((s) => s.currentFolderId);
	const breadcrumb = useFileStore((s) => s.breadcrumb);
	const [targetDialogMode, setTargetDialogMode] = useState<
		"move" | "copy" | null
	>(null);
	const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);

	const count = selectedFileIds.size + selectedFolderIds.size;
	if (count === 0) return null;

	const fileIds = Array.from(selectedFileIds);
	const folderIds = Array.from(selectedFolderIds);

	const handleDelete = async () => {
		setDeleteConfirmOpen(false);
		try {
			const result = await batchService.batchDelete(fileIds, folderIds);
			const batchToast = formatBatchToast(t, "delete", result);
			if (batchToast.variant === "error") {
				toast.error(batchToast.title, { description: batchToast.description });
			} else {
				toast.success(batchToast.title, {
					description: batchToast.description,
				});
			}
			clearSelection();
			await refresh();
		} catch (err) {
			handleApiError(err);
		}
	};

	const handleMove = () => {
		setTargetDialogMode("move");
	};

	const handleCopy = () => {
		setTargetDialogMode("copy");
	};

	const handleTargetConfirm = async (targetFolderId: number | null) => {
		if (!targetDialogMode) return;

		try {
			const result =
				targetDialogMode === "move"
					? await moveToFolder(fileIds, folderIds, targetFolderId)
					: await batchService.batchCopy(fileIds, folderIds, targetFolderId);
			const batchToast = formatBatchToast(t, targetDialogMode, result);
			if (batchToast.variant === "error") {
				toast.error(batchToast.title, { description: batchToast.description });
			} else {
				toast.success(batchToast.title, {
					description: batchToast.description,
				});
			}
			if (targetDialogMode === "copy") {
				clearSelection();
				await refresh();
			}
			setTargetDialogMode(null);
		} catch (err) {
			handleApiError(err);
		}
	};

	return (
		<>
			<div className="fixed bottom-4 left-1/2 -translate-x-1/2 z-50 flex items-center gap-2 bg-background border rounded-lg shadow-lg px-4 py-2">
				<span className="text-sm font-medium">
					{t("common:selected_count", { count })}
				</span>
				<div className="flex items-center gap-1">
					<Button
						size="sm"
						variant="destructive"
						onClick={() => setDeleteConfirmOpen(true)}
					>
						<Icon name="Trash" className="h-3.5 w-3.5 mr-1" />
						{t("common:delete")}
					</Button>
					<Button size="sm" variant="outline" onClick={handleMove}>
						<Icon name="ArrowsOutCardinal" className="h-3.5 w-3.5 mr-1" />
						{t("move")}
					</Button>
					<Button size="sm" variant="outline" onClick={handleCopy}>
						<Icon name="Copy" className="h-3.5 w-3.5 mr-1" />
						{t("copy")}
					</Button>
				</div>
				<Button size="sm" variant="ghost" onClick={clearSelection}>
					<Icon name="X" className="h-3.5 w-3.5" />
				</Button>
			</div>

			<ConfirmDialog
				open={deleteConfirmOpen}
				onOpenChange={setDeleteConfirmOpen}
				title={t("batch_delete_confirm_title", { count })}
				description={t("batch_delete_confirm_desc")}
				confirmLabel={t("common:delete")}
				variant="destructive"
				onConfirm={handleDelete}
			/>

			<BatchTargetFolderDialog
				open={targetDialogMode !== null}
				onOpenChange={(open) => {
					if (!open) setTargetDialogMode(null);
				}}
				mode={targetDialogMode ?? "move"}
				onConfirm={handleTargetConfirm}
				currentFolderId={currentFolderId}
				initialBreadcrumb={breadcrumb as BreadcrumbItem[]}
				selectedFolderIds={folderIds}
			/>
		</>
	);
}
