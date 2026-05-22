import { describe, expect, it } from "vitest";
import {
	filePreviewDialogUiReducer,
	initialFilePreviewDialogUiState,
} from "./filePreviewDialogState";

describe("filePreviewDialogUiReducer", () => {
	it("resets file-scoped UI state when the file changes", () => {
		const selected = filePreviewDialogUiReducer(
			initialFilePreviewDialogUiState,
			{ type: "selectOpenMethod", mode: "builtin.markdown" },
		);
		const expanded = filePreviewDialogUiReducer(selected, {
			type: "toggleExpanded",
		});
		const reset = filePreviewDialogUiReducer(expanded, {
			type: "syncMode",
			preferredMode: "builtin.code",
			resetForFile: true,
		});

		expect(reset).toMatchObject({
			forceOpenMethodChooser: false,
			hasConfirmedInitialMode: false,
			isExpanded: false,
			mode: "builtin.code",
		});
	});

	it("keeps file-scoped UI state when only preferred mode refreshes", () => {
		const confirmed = filePreviewDialogUiReducer(
			initialFilePreviewDialogUiState,
			{ type: "selectOpenMethod", mode: "builtin.markdown" },
		);
		const synced = filePreviewDialogUiReducer(confirmed, {
			type: "syncMode",
			preferredMode: "builtin.code",
			resetForFile: false,
		});

		expect(synced).toMatchObject({
			hasConfirmedInitialMode: true,
			mode: "builtin.code",
		});
	});

	it("opens the method picker and clears expanded hidden methods", () => {
		const showingAll = filePreviewDialogUiReducer(
			initialFilePreviewDialogUiState,
			{ type: "showAllOpenMethods" },
		);
		const picker = filePreviewDialogUiReducer(showingAll, {
			type: "openMethodPickerOpened",
		});

		expect(picker).toMatchObject({
			forceOpenMethodChooser: true,
			hasConfirmedInitialMode: false,
			isDialogAnimationEnabled: true,
			showAllOpenMethods: false,
		});
	});

	it("tracks dirty confirmation and discard state together", () => {
		const dirty = filePreviewDialogUiReducer(initialFilePreviewDialogUiState, {
			type: "setDirty",
			isDirty: true,
		});
		const confirming = filePreviewDialogUiReducer(dirty, {
			type: "setConfirmOpen",
			confirmOpen: true,
		});
		const discarded = filePreviewDialogUiReducer(confirming, {
			type: "discardChanges",
		});

		expect(discarded).toMatchObject({
			confirmOpen: false,
			isDirty: false,
		});
	});
});
