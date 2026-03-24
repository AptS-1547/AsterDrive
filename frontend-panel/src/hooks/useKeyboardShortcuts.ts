import { useEffect } from "react";
import {
	shouldIgnoreKeyboardTarget,
	useSelectionShortcuts,
} from "@/hooks/useSelectionShortcuts";
import { useFileStore } from "@/stores/fileStore";

export function useKeyboardShortcuts() {
	const selectAll = useFileStore((s) => s.selectAll);
	const clearSelection = useFileStore((s) => s.clearSelection);

	useSelectionShortcuts({ selectAll, clearSelection });

	useEffect(() => {
		function handleKeyDown(e: KeyboardEvent) {
			if (shouldIgnoreKeyboardTarget(e.target)) return;

			const mod = e.metaKey || e.ctrlKey;

			// / or Ctrl+K: Focus search input
			if (e.key === "/" || (mod && e.key.toLowerCase() === "k")) {
				e.preventDefault();
				const searchInput = document.querySelector<HTMLInputElement>(
					"[data-search-input]",
				);
				if (searchInput) {
					searchInput.focus();
				}
			}
		}

		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, []);
}
