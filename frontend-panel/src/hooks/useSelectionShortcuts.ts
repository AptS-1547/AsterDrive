import { useEffect } from "react";
import {
	isImeComposingKeyEvent,
	shouldIgnoreKeyboardTarget,
} from "@/lib/keyboard";

interface UseSelectionShortcutsOptions {
	selectAll: () => void;
	clearSelection: () => void;
	enabled?: boolean;
}

export { shouldIgnoreKeyboardTarget } from "@/lib/keyboard";

export function useSelectionShortcuts({
	selectAll,
	clearSelection,
	enabled = true,
}: UseSelectionShortcutsOptions) {
	useEffect(() => {
		if (!enabled) return;

		function handleKeyDown(e: KeyboardEvent) {
			if (shouldIgnoreKeyboardTarget(e.target) || isImeComposingKeyEvent(e)) {
				return;
			}

			const mod = e.metaKey || e.ctrlKey;
			if (mod && e.key.toLowerCase() === "a") {
				e.preventDefault();
				selectAll();
			}

			if (e.key === "Escape") {
				clearSelection();
			}
		}

		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, [clearSelection, enabled, selectAll]);
}
