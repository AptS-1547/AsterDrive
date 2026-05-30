import { useCallback, useReducer } from "react";
import type { MyShareInfo } from "@/types/api";

interface MySharesPageState {
	editTarget: MyShareInfo | null;
	loading: boolean;
	page: number;
	selectedShareIds: Set<number>;
	shares: MyShareInfo[];
	total: number;
}

type MySharesPageAction =
	| { type: "loadStarted" }
	| { type: "loadFinished" }
	| { type: "pageLoaded"; items: MyShareInfo[]; total: number }
	| { type: "pageChanged"; page: number }
	| { type: "pageUpdated"; updater: (page: number) => number }
	| { type: "selectionCleared" }
	| { type: "allSelected"; shareIds: number[] }
	| { type: "shareSelectionToggled"; shareId: number }
	| { type: "editTargetChanged"; share: MyShareInfo | null };

const initialMySharesPageState: MySharesPageState = {
	editTarget: null,
	loading: true,
	page: 0,
	selectedShareIds: new Set(),
	shares: [],
	total: 0,
};

function mySharesPageReducer(
	state: MySharesPageState,
	action: MySharesPageAction,
): MySharesPageState {
	switch (action.type) {
		case "loadStarted":
			return { ...state, loading: true };
		case "loadFinished":
			return { ...state, loading: false };
		case "pageLoaded":
			return {
				...state,
				selectedShareIds: new Set(),
				shares: action.items,
				total: action.total,
			};
		case "pageChanged":
			return { ...state, page: action.page };
		case "pageUpdated":
			return { ...state, page: action.updater(state.page) };
		case "selectionCleared":
			return { ...state, selectedShareIds: new Set() };
		case "allSelected":
			return { ...state, selectedShareIds: new Set(action.shareIds) };
		case "shareSelectionToggled": {
			const selectedShareIds = new Set(state.selectedShareIds);
			if (selectedShareIds.has(action.shareId)) {
				selectedShareIds.delete(action.shareId);
			} else {
				selectedShareIds.add(action.shareId);
			}
			return { ...state, selectedShareIds };
		}
		case "editTargetChanged":
			return { ...state, editTarget: action.share };
	}
}

export function useMySharesPageState() {
	const [state, dispatch] = useReducer(
		mySharesPageReducer,
		initialMySharesPageState,
	);
	const clearSelection = useCallback(
		() => dispatch({ type: "selectionCleared" }),
		[],
	);
	const finishLoading = useCallback(
		() => dispatch({ type: "loadFinished" }),
		[],
	);
	const selectAll = useCallback(
		(shareIds: number[]) => dispatch({ type: "allSelected", shareIds }),
		[],
	);
	const setEditTarget = useCallback(
		(share: MyShareInfo | null) =>
			dispatch({ type: "editTargetChanged", share }),
		[],
	);
	const setPage = useCallback(
		(updater: number | ((page: number) => number)) =>
			typeof updater === "function"
				? dispatch({ type: "pageUpdated", updater })
				: dispatch({ type: "pageChanged", page: updater }),
		[],
	);
	const setPageData = useCallback(
		(items: MyShareInfo[], total: number) =>
			dispatch({ type: "pageLoaded", items, total }),
		[],
	);
	const startLoading = useCallback(() => dispatch({ type: "loadStarted" }), []);
	const toggleShareSelection = useCallback(
		(shareId: number) => dispatch({ type: "shareSelectionToggled", shareId }),
		[],
	);

	return {
		...state,
		clearSelection,
		finishLoading,
		selectAll,
		setEditTarget,
		setPage,
		setPageData,
		startLoading,
		toggleShareSelection,
	};
}
