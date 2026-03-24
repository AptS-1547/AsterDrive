import { useEffect } from "react";

interface UseSelectionShortcutsOptions {
	selectAll: () => void;
	clearSelection: () => void;
	enabled?: boolean;
}

export function shouldIgnoreKeyboardTarget(target: EventTarget | null) {
	if (!(target instanceof HTMLElement)) return false;

	return (
		target.tagName === "INPUT" ||
		target.tagName === "TEXTAREA" ||
		target.tagName === "SELECT" ||
		target.isContentEditable
	);
}

export function useSelectionShortcuts({
	selectAll,
	clearSelection,
	enabled = true,
}: UseSelectionShortcutsOptions) {
	useEffect(() => {
		if (!enabled) return;

		function handleKeyDown(e: KeyboardEvent) {
			if (shouldIgnoreKeyboardTarget(e.target)) return;

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
