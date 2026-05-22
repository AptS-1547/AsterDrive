import type { OpenWithMode } from "./types";

export type FilePreviewDialogUiState = {
	mode: OpenWithMode | null;
	isDialogAnimationEnabled: boolean;
	isExpanded: boolean;
	hasConfirmedInitialMode: boolean;
	forceOpenMethodChooser: boolean;
	isDirty: boolean;
	confirmOpen: boolean;
	showAllOpenMethods: boolean;
};
export type FilePreviewDialogUiAction =
	| {
			type: "syncMode";
			preferredMode: OpenWithMode | null;
			resetForFile: boolean;
	  }
	| { type: "syncShowAllOpenMethods"; showAllOpenMethods: boolean }
	| { type: "selectOpenMethod"; mode: OpenWithMode }
	| { type: "openMethodPickerOpened" }
	| { type: "setConfirmOpen"; confirmOpen: boolean }
	| { type: "setDirty"; isDirty: boolean }
	| { type: "discardChanges" }
	| { type: "toggleExpanded" }
	| { type: "disableAnimation" }
	| { type: "showAllOpenMethods" };

export const initialFilePreviewDialogUiState: FilePreviewDialogUiState = {
	mode: null,
	isDialogAnimationEnabled: true,
	isExpanded: false,
	hasConfirmedInitialMode: false,
	forceOpenMethodChooser: false,
	isDirty: false,
	confirmOpen: false,
	showAllOpenMethods: false,
};

export function filePreviewDialogUiReducer(
	state: FilePreviewDialogUiState,
	action: FilePreviewDialogUiAction,
): FilePreviewDialogUiState {
	switch (action.type) {
		case "syncMode":
			return {
				...state,
				mode: action.preferredMode,
				hasConfirmedInitialMode: action.resetForFile
					? false
					: state.hasConfirmedInitialMode,
				isExpanded: action.resetForFile ? false : state.isExpanded,
				forceOpenMethodChooser: action.resetForFile
					? false
					: state.forceOpenMethodChooser,
			};
		case "syncShowAllOpenMethods":
			if (state.showAllOpenMethods === action.showAllOpenMethods) {
				return state;
			}
			return {
				...state,
				showAllOpenMethods: action.showAllOpenMethods,
			};
		case "selectOpenMethod":
			return {
				...state,
				mode: action.mode,
				isDialogAnimationEnabled: true,
				forceOpenMethodChooser: false,
				hasConfirmedInitialMode: true,
			};
		case "openMethodPickerOpened":
			return {
				...state,
				isDialogAnimationEnabled: true,
				forceOpenMethodChooser: true,
				hasConfirmedInitialMode: false,
				showAllOpenMethods: false,
			};
		case "setConfirmOpen":
			if (state.confirmOpen === action.confirmOpen) {
				return state;
			}
			return {
				...state,
				confirmOpen: action.confirmOpen,
			};
		case "setDirty":
			if (state.isDirty === action.isDirty) {
				return state;
			}
			return {
				...state,
				isDirty: action.isDirty,
			};
		case "discardChanges":
			return {
				...state,
				confirmOpen: false,
				isDirty: false,
			};
		case "toggleExpanded":
			return {
				...state,
				isDialogAnimationEnabled: false,
				isExpanded: !state.isExpanded,
			};
		case "disableAnimation":
			if (!state.isDialogAnimationEnabled) {
				return state;
			}
			return {
				...state,
				isDialogAnimationEnabled: false,
			};
		case "showAllOpenMethods":
			if (state.showAllOpenMethods) {
				return state;
			}
			return {
				...state,
				showAllOpenMethods: true,
			};
	}
}
