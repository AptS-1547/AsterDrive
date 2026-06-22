import { useEffect, useMemo } from "react";
import type { FilePreviewDialogUiAction } from "./filePreviewDialogState";
import type { FilePreviewProfile, OpenWithMode, OpenWithOption } from "./types";

interface UsePreviewOpenModeOptions {
	allOptions: OpenWithOption[];
	dispatch: React.Dispatch<FilePreviewDialogUiAction>;
	fileId: number;
	forceOpenMethodChooser: boolean;
	hasConfirmedInitialMode: boolean;
	hiddenOptions: OpenWithOption[];
	openMode: "auto" | "direct" | "picker";
	previewAppsLoaded: boolean;
	profile: FilePreviewProfile | null;
	stateMode: OpenWithMode | null;
	visibleOptions: OpenWithOption[];
}

export function usePreviewOpenMode({
	allOptions,
	dispatch,
	fileId,
	forceOpenMethodChooser,
	hasConfirmedInitialMode,
	hiddenOptions,
	openMode,
	previewAppsLoaded,
	profile,
	stateMode,
	visibleOptions,
}: UsePreviewOpenModeOptions) {
	const preferredMode = useMemo(() => {
		if (!profile) return null;
		if (
			profile.defaultMode &&
			allOptions.some((option) => option.key === profile.defaultMode)
		) {
			return profile.defaultMode;
		}
		return allOptions[0]?.key ?? null;
	}, [allOptions, profile]);

	useEffect(() => {
		dispatch({
			type: "syncMode",
			fileId,
			preferredMode,
		});
	}, [dispatch, fileId, preferredMode]);

	const activeMode = stateMode ?? preferredMode;

	useEffect(() => {
		dispatch({
			type: "syncShowAllOpenMethods",
			showAllOpenMethods: Boolean(
				activeMode && hiddenOptions.some((option) => option.key === activeMode),
			),
		});
	}, [activeMode, dispatch, hiddenOptions]);

	const activeOption = useMemo(() => {
		if (!profile || !activeMode) return null;
		return allOptions.find((option) => option.key === activeMode) ?? null;
	}, [activeMode, allOptions, profile]);

	const shouldAutoOpenPreferredMode =
		openMode === "auto" &&
		Boolean(profile) &&
		profile?.category === "image" &&
		profile.isTextBased &&
		allOptions.some(
			(option) => option.key === preferredMode && option.mode === "image",
		);
	const hasMultipleVisibleOpenMethods = visibleOptions.length > 1;
	const showOpenMethodChooser =
		previewAppsLoaded &&
		(forceOpenMethodChooser
			? allOptions.length > 1
			: openMode === "picker"
				? allOptions.length > 1
				: openMode === "direct"
					? false
					: shouldAutoOpenPreferredMode
						? false
						: hasMultipleVisibleOpenMethods) &&
		!hasConfirmedInitialMode;

	return {
		activeMode,
		activeOption,
		showOpenMethodChooser,
	};
}
