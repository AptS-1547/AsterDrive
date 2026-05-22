import { useCallback, useEffect, useMemo, useReducer, useRef } from "react";
import { fileService } from "@/services/fileService";
import { usePreviewAppStore } from "@/stores/previewAppStore";
import type { FileInfo, FileListItem } from "@/types/api";
import { detectFilePreviewProfile } from "./file-capabilities";
import {
	filePreviewDialogUiReducer,
	initialFilePreviewDialogUiState,
} from "./filePreviewDialogState";
import type { FilePreviewDialogProps } from "./filePreviewDialogTypes";
import type { OpenWithMode, OpenWithOption } from "./types";
import { getVideoBrowserOpenWithOption } from "./video-browser-config";

const PREVIEW_DIALOG_OPEN_ANIMATION_MS = 120;

export function useResolvedPreviewSources({
	file,
	downloadPath,
	imagePreviewPath,
	thumbnailPath,
	loadMusicBackendMetadata,
}: Pick<
	FilePreviewDialogProps,
	| "downloadPath"
	| "imagePreviewPath"
	| "thumbnailPath"
	| "loadMusicBackendMetadata"
> & {
	file: FileInfo | FileListItem;
}) {
	return {
		resolvedDownloadPath: downloadPath ?? fileService.downloadPath(file.id),
		resolvedImagePreviewPath:
			imagePreviewPath ?? fileService.imagePreviewPath(file.id),
		resolvedThumbnailPath: thumbnailPath ?? fileService.thumbnailPath(file.id),
		resolvedLoadMusicBackendMetadata:
			loadMusicBackendMetadata ??
			((signal?: AbortSignal) =>
				import("@/lib/musicPlayer").then(
					({ backendAudioMetadataToTrackMetadata }) =>
						fileService
							.getMediaMetadata(file.id, { signal })
							.then((metadata) =>
								backendAudioMetadataToTrackMetadata(metadata),
							),
				)),
	};
}

export function useFilePreviewOptions({
	file,
	archivePreviewFactory,
	wopiSessionFactory,
	openMode,
}: {
	file: FileInfo | FileListItem;
	archivePreviewFactory: FilePreviewDialogProps["archivePreviewFactory"];
	wopiSessionFactory: FilePreviewDialogProps["wopiSessionFactory"];
	openMode: NonNullable<FilePreviewDialogProps["openMode"]>;
}) {
	const previewApps = usePreviewAppStore((state) => state.config);
	const previewAppsLoaded = usePreviewAppStore((state) => state.isLoaded);
	const loadPreviewApps = usePreviewAppStore((state) => state.load);

	useEffect(() => {
		if (previewAppsLoaded) return;
		void loadPreviewApps();
	}, [loadPreviewApps, previewAppsLoaded]);

	const baseProfile = useMemo(() => {
		if (!previewAppsLoaded) return null;
		return detectFilePreviewProfile(file, previewApps);
	}, [file, previewApps, previewAppsLoaded]);
	const customVideoBrowserOption = useMemo(
		() => getVideoBrowserOpenWithOption(),
		[],
	);
	const profile = useMemo(() => {
		if (!baseProfile) return null;
		if (
			baseProfile.category !== "video" ||
			!customVideoBrowserOption ||
			baseProfile.options.some(
				(option) => option.key === customVideoBrowserOption.key,
			)
		) {
			return baseProfile;
		}

		return {
			...baseProfile,
			options: [...baseProfile.options, customVideoBrowserOption],
			allOptions: [
				...(baseProfile.allOptions ?? baseProfile.options),
				customVideoBrowserOption,
			],
		};
	}, [baseProfile, customVideoBrowserOption]);
	const isOptionAvailable = useCallback(
		(option: OpenWithOption) =>
			(option.mode !== "wopi" || Boolean(wopiSessionFactory)) &&
			(option.mode !== "archive" || Boolean(archivePreviewFactory)),
		[archivePreviewFactory, wopiSessionFactory],
	);
	const allOptions = useMemo(
		() =>
			(profile?.allOptions ?? profile?.options ?? []).filter(isOptionAvailable),
		[isOptionAvailable, profile],
	);
	const visibleOptions = useMemo(() => {
		if (!profile || profile.options.length === 0) {
			return allOptions;
		}

		const nextVisibleOptions = profile.options.filter(isOptionAvailable);
		return nextVisibleOptions.length > 0 ? nextVisibleOptions : allOptions;
	}, [allOptions, isOptionAvailable, profile]);
	const hiddenOptions = useMemo(
		() =>
			allOptions.filter(
				(option) =>
					!visibleOptions.some((candidate) => candidate.key === option.key),
			),
		[allOptions, visibleOptions],
	);
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
	const shouldAutoOpenPreferredMode = useMemo(
		() =>
			openMode === "auto" &&
			Boolean(profile) &&
			profile?.category === "image" &&
			profile.isTextBased &&
			allOptions.some(
				(option) => option.key === preferredMode && option.mode === "image",
			),
		[allOptions, openMode, preferredMode, profile],
	);

	return {
		allOptions,
		hiddenOptions,
		preferredMode,
		previewAppsLoaded,
		profile,
		shouldAutoOpenPreferredMode,
		visibleOptions,
	};
}

export function useFilePreviewDialogUi({
	allOptionsCount,
	fileId,
	hasMultipleVisibleOpenMethods,
	hiddenOptions,
	onClose,
	open,
	openMode,
	preferredMode,
	previewAppsLoaded,
	shouldAutoOpenPreferredMode,
}: {
	allOptionsCount: number;
	fileId: FileInfo["id"] | FileListItem["id"];
	hasMultipleVisibleOpenMethods: boolean;
	hiddenOptions: OpenWithOption[];
	onClose: () => void;
	open: boolean;
	openMode: NonNullable<FilePreviewDialogProps["openMode"]>;
	preferredMode: OpenWithMode | null;
	previewAppsLoaded: boolean;
	shouldAutoOpenPreferredMode: boolean;
}) {
	const [state, dispatch] = useReducer(
		filePreviewDialogUiReducer,
		initialFilePreviewDialogUiState,
	);
	const previousFileIdRef = useRef(fileId);
	const activeMode = state.mode ?? preferredMode;
	const showOpenMethodChooser =
		previewAppsLoaded &&
		(state.forceOpenMethodChooser
			? allOptionsCount > 1
			: openMode === "picker"
				? allOptionsCount > 1
				: openMode === "direct"
					? false
					: shouldAutoOpenPreferredMode
						? false
						: hasMultipleVisibleOpenMethods) &&
		!state.hasConfirmedInitialMode;

	useEffect(() => {
		const hasFileChanged = previousFileIdRef.current !== fileId;
		if (hasFileChanged) {
			previousFileIdRef.current = fileId;
		}
		dispatch({
			type: "syncMode",
			preferredMode,
			resetForFile: hasFileChanged,
		});
	}, [fileId, preferredMode]);

	useEffect(() => {
		dispatch({
			type: "syncShowAllOpenMethods",
			showAllOpenMethods: Boolean(
				activeMode && hiddenOptions.some((option) => option.key === activeMode),
			),
		});
	}, [activeMode, hiddenOptions]);

	const closeWithGuard = useCallback(() => {
		if (state.isDirty) {
			dispatch({ type: "setConfirmOpen", confirmOpen: true });
			return;
		}
		onClose();
	}, [onClose, state.isDirty]);
	const handleOpenMethodSelect = useCallback((nextMode: OpenWithMode) => {
		dispatch({ type: "selectOpenMethod", mode: nextMode });
	}, []);
	const handleOpenMethodPickerOpen = useCallback(() => {
		dispatch({ type: "openMethodPickerOpened" });
	}, []);
	const handleDiscardChanges = useCallback(() => {
		dispatch({ type: "discardChanges" });
		onClose();
	}, [onClose]);
	const handleExpandToggle = useCallback(() => {
		dispatch({ type: "toggleExpanded" });
	}, []);
	const handleDirtyChange = useCallback((dirty: boolean) => {
		dispatch({ type: "setDirty", isDirty: dirty });
	}, []);
	const handleShowAllOpenMethods = useCallback(() => {
		dispatch({ type: "showAllOpenMethods" });
	}, []);
	const handleConfirmOpenChange = useCallback((confirmOpen: boolean) => {
		dispatch({ type: "setConfirmOpen", confirmOpen });
	}, []);

	useEffect(() => {
		if (!open || showOpenMethodChooser || !state.isDialogAnimationEnabled) {
			return;
		}

		const timer = window.setTimeout(() => {
			dispatch({ type: "disableAnimation" });
		}, PREVIEW_DIALOG_OPEN_ANIMATION_MS);

		return () => {
			window.clearTimeout(timer);
		};
	}, [open, showOpenMethodChooser, state.isDialogAnimationEnabled]);

	const handleDialogOpenChange = useCallback(
		(open: boolean) => {
			if (open) {
				return;
			}

			if (showOpenMethodChooser) {
				onClose();
				return;
			}

			closeWithGuard();
		},
		[closeWithGuard, onClose, showOpenMethodChooser],
	);

	return {
		activeMode,
		closeWithGuard,
		handleConfirmOpenChange,
		handleDialogOpenChange,
		handleDirtyChange,
		handleDiscardChanges,
		handleExpandToggle,
		handleOpenMethodPickerOpen,
		handleOpenMethodSelect,
		handleShowAllOpenMethods,
		showOpenMethodChooser,
		state,
	};
}

export function useActiveArchivePreviewFactory({
	activeOption,
	archivePreviewFactory,
	open,
}: {
	activeOption: OpenWithOption | null;
	archivePreviewFactory: FilePreviewDialogProps["archivePreviewFactory"];
	open: boolean;
}) {
	const archivePreviewFactoryRef = useRef(archivePreviewFactory);
	useEffect(() => {
		archivePreviewFactoryRef.current = archivePreviewFactory;
	}, [archivePreviewFactory]);

	const stableArchivePreviewFactory = useCallback(
		(options?: Parameters<NonNullable<typeof archivePreviewFactory>>[0]) => {
			const factory = archivePreviewFactoryRef.current;
			if (!factory) {
				return Promise.reject(new Error("archive preview factory unavailable"));
			}

			return factory(options);
		},
		[],
	);

	return open && activeOption?.mode === "archive" && archivePreviewFactory
		? stableArchivePreviewFactory
		: undefined;
}
