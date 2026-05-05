import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { handleApiError } from "@/hooks/useApiError";
import {
	shouldIgnoreKeyboardTarget,
	useSelectionShortcuts,
} from "@/hooks/useSelectionShortcuts";
import type { BatchOperation } from "@/lib/formatBatchToast";
import { formatBatchToast } from "@/lib/formatBatchToast";
import { isImeComposingKeyEvent } from "@/lib/keyboard";
import { useFileStore } from "@/stores/fileStore";

export function useKeyboardShortcuts() {
	const { t } = useTranslation("files");
	const selectAll = useFileStore((s) => s.selectAll);
	const clearSelection = useFileStore((s) => s.clearSelection);

	useSelectionShortcuts({ selectAll, clearSelection });

	useEffect(() => {
		function handleKeyDown(e: KeyboardEvent) {
			if (shouldIgnoreKeyboardTarget(e.target) || isImeComposingKeyEvent(e)) {
				return;
			}

			const mod = e.metaKey || e.ctrlKey;
			const key = e.key.toLowerCase();

			// Ctrl+C: Copy selection to clipboard
			if (mod && key === "c") {
				const count = useFileStore.getState().clipboardCopy();
				if (count > 0) {
					e.preventDefault();
					toast.info(t("clipboard_copied", { count }));
				}
				return;
			}

			// Ctrl+X: Cut selection to clipboard
			if (mod && key === "x") {
				const count = useFileStore.getState().clipboardCut();
				if (count > 0) {
					e.preventDefault();
					toast.info(t("clipboard_cut", { count }));
				}
				return;
			}

			// Ctrl+V: Paste clipboard to current folder
			if (mod && key === "v") {
				const clipboard = useFileStore.getState().clipboard;
				if (!clipboard) return;
				e.preventDefault();
				void (async () => {
					try {
						const { mode, result } = await useFileStore
							.getState()
							.clipboardPaste();
						const operation: BatchOperation = mode === "copy" ? "copy" : "move";
						const batchToast = formatBatchToast(t, operation, result);
						if (batchToast.variant === "error") {
							toast.error(batchToast.title, {
								description: batchToast.description,
							});
						} else {
							toast.success(batchToast.title, {
								description: batchToast.description,
							});
						}
					} catch (err) {
						handleApiError(err);
					}
				})();
				return;
			}

			// F2: Rename single selected item
			if (e.key === "F2") {
				const { selectedFileIds, selectedFolderIds, files, folders } =
					useFileStore.getState();
				if (selectedFileIds.size === 1 && selectedFolderIds.size === 0) {
					const id = [...selectedFileIds][0];
					const file = files.find((f) => f.id === id);
					if (file) {
						e.preventDefault();
						document.dispatchEvent(
							new CustomEvent("rename-request", {
								detail: { type: "file", id, name: file.name },
							}),
						);
					}
				} else if (selectedFolderIds.size === 1 && selectedFileIds.size === 0) {
					const id = [...selectedFolderIds][0];
					const folder = folders.find((f) => f.id === id);
					if (folder) {
						e.preventDefault();
						document.dispatchEvent(
							new CustomEvent("rename-request", {
								detail: { type: "folder", id, name: folder.name },
							}),
						);
					}
				}
				return;
			}
		}

		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, [t]);
}
