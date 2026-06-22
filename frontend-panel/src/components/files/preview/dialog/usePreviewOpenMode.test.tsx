import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { FilePreviewProfile, OpenWithOption } from "../capabilities/types";
import {
	filePreviewDialogUiReducer,
	initialFilePreviewDialogUiState,
} from "./filePreviewDialogState";
import { usePreviewOpenMode } from "./usePreviewOpenMode";

const imageOption: OpenWithOption = {
	icon: "Image",
	key: "builtin.image",
	labelKey: "open_with_image",
	mode: "image",
};
const codeOption: OpenWithOption = {
	icon: "TextT",
	key: "builtin.code",
	labelKey: "open_with_code",
	mode: "code",
};
const archiveOption: OpenWithOption = {
	icon: "Archive",
	key: "builtin.archive",
	labelKey: "open_with_archive",
	mode: "archive",
};

function profile(
	overrides: Partial<FilePreviewProfile> = {},
): FilePreviewProfile {
	return {
		category: "image",
		defaultMode: "builtin.image",
		isBlobPreview: true,
		isEditableText: false,
		isTextBased: true,
		options: [imageOption],
		...overrides,
	};
}

function renderOpenMode(
	overrides: Partial<Parameters<typeof usePreviewOpenMode>[0]> = {},
) {
	const dispatch = vi.fn();
	const props: Parameters<typeof usePreviewOpenMode>[0] = {
		allOptions: [imageOption],
		dispatch,
		fileId: 7,
		forceOpenMethodChooser: false,
		hasConfirmedInitialMode: false,
		hiddenOptions: [],
		openMode: "auto",
		previewAppsLoaded: true,
		profile: profile(),
		stateMode: null,
		visibleOptions: [imageOption],
		...overrides,
	};
	const hook = renderHook(
		(nextProps: typeof props) => usePreviewOpenMode(nextProps),
		{ initialProps: props },
	);
	return { ...hook, dispatch };
}

describe("usePreviewOpenMode", () => {
	it("selects the preferred mode and hides the chooser for auto-open image previews", () => {
		const { dispatch, result } = renderOpenMode();

		expect(result.current.activeMode).toBe("builtin.image");
		expect(result.current.activeOption).toBe(imageOption);
		expect(result.current.showOpenMethodChooser).toBe(false);
		expect(dispatch).toHaveBeenCalledWith({
			type: "syncMode",
			fileId: 7,
			preferredMode: "builtin.image",
		});
	});

	it("opens the chooser when picker mode has multiple options", () => {
		const { result } = renderOpenMode({
			allOptions: [codeOption, archiveOption],
			openMode: "picker",
			profile: profile({
				category: "markdown",
				defaultMode: "builtin.code",
				isBlobPreview: false,
				isEditableText: true,
				options: [codeOption, archiveOption],
			}),
			visibleOptions: [codeOption, archiveOption],
		});

		expect(result.current.activeMode).toBe("builtin.code");
		expect(result.current.showOpenMethodChooser).toBe(true);
	});

	it("syncs hidden option expansion when the active mode is not visible", () => {
		const { dispatch, result } = renderOpenMode({
			allOptions: [codeOption, archiveOption],
			hiddenOptions: [archiveOption],
			profile: profile({
				category: "archive",
				defaultMode: "builtin.archive",
				isBlobPreview: false,
				isEditableText: false,
				isTextBased: false,
				options: [codeOption],
			}),
			stateMode: "builtin.archive",
			visibleOptions: [codeOption],
		});

		expect(result.current.activeOption).toBe(archiveOption);
		expect(dispatch).toHaveBeenCalledWith({
			type: "syncShowAllOpenMethods",
			showAllOpenMethods: true,
		});
	});

	it("keeps confirmed selections from reopening the chooser", () => {
		const state = filePreviewDialogUiReducer(initialFilePreviewDialogUiState, {
			type: "selectOpenMethod",
			mode: "builtin.code",
		});
		const { result } = renderOpenMode({
			allOptions: [codeOption, archiveOption],
			hasConfirmedInitialMode: state.hasConfirmedInitialMode,
			openMode: "picker",
			profile: profile({
				category: "markdown",
				defaultMode: "builtin.code",
				isBlobPreview: false,
				isEditableText: true,
				options: [codeOption, archiveOption],
			}),
			stateMode: state.mode,
			visibleOptions: [codeOption, archiveOption],
		});

		expect(result.current.showOpenMethodChooser).toBe(false);
	});
});
