import { useEffect, useRef } from "react";
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
	const selectAllRef = useRef(selectAll);
	const clearSelectionRef = useRef(clearSelection);

	useEffect(() => {
		selectAllRef.current = selectAll;
		clearSelectionRef.current = clearSelection;
	}, [clearSelection, selectAll]);

	useEffect(() => {
		if (!enabled) return;

		function handleKeyDown(e: KeyboardEvent) {
			if (shouldIgnoreKeyboardTarget(e.target) || isImeComposingKeyEvent(e)) {
				return;
			}

			const mod = e.metaKey || e.ctrlKey;
			if (mod && e.key.toLowerCase() === "a") {
				e.preventDefault();
				selectAllRef.current();
			}

			if (e.key === "Escape") {
				clearSelectionRef.current();
			}
		}

		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, [enabled]);
}
