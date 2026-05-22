import { useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import {
	FilePreviewChooserContent,
	FilePreviewPanelContent,
} from "./FilePreviewDialogContent";
import type { FilePreviewDialogProps } from "./filePreviewDialogTypes";
import {
	getDialogContentClassName,
	optionFillsViewportHeight,
	optionUsesInnerScroll,
} from "./filePreviewDialogUtils";
import { resolveOpenWithOptionLabel } from "./openWithLabel";
import type { OpenWithOption } from "./types";
import { UnsavedChangesGuard } from "./UnsavedChangesGuard";
import {
	useActiveArchivePreviewFactory,
	useFilePreviewDialogUi,
	useFilePreviewOptions,
	useResolvedPreviewSources,
} from "./useFilePreviewDialog";

export function FilePreviewDialog({
	open,
	file,
	onClose,
	onOpenChangeComplete,
	onFileUpdated,
	downloadPath,
	imagePreviewPath,
	thumbnailPath,
	editable = true,
	previewLinkFactory,
	archivePreviewFactory,
	loadMusicBackendMetadata,
	mediaStreamLinkFactory,
	wopiSessionFactory,
	openMode = "auto",
}: FilePreviewDialogProps) {
	const { i18n, t } = useTranslation(["core", "files"]);
	const {
		resolvedDownloadPath,
		resolvedImagePreviewPath,
		resolvedThumbnailPath,
		resolvedLoadMusicBackendMetadata,
	} = useResolvedPreviewSources({
		file,
		downloadPath,
		imagePreviewPath,
		thumbnailPath,
		loadMusicBackendMetadata,
	});
	const {
		allOptions,
		hiddenOptions,
		preferredMode,
		previewAppsLoaded,
		profile,
		shouldAutoOpenPreferredMode,
		visibleOptions,
	} = useFilePreviewOptions({
		file,
		archivePreviewFactory,
		wopiSessionFactory,
		openMode,
	});
	const {
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
		state: uiState,
	} = useFilePreviewDialogUi({
		allOptionsCount: allOptions.length,
		fileId: file.id,
		hasMultipleVisibleOpenMethods: visibleOptions.length > 1,
		hiddenOptions,
		onClose,
		open,
		openMode,
		preferredMode,
		previewAppsLoaded,
		shouldAutoOpenPreferredMode,
	});
	const {
		isDialogAnimationEnabled,
		isExpanded,
		isDirty,
		confirmOpen,
		showAllOpenMethods,
	} = uiState;

	const activeOption = useMemo(() => {
		if (!profile || !activeMode) return null;
		return allOptions.find((option) => option.key === activeMode) ?? null;
	}, [activeMode, allOptions, profile]);
	const getOptionLabel = useCallback(
		(option: OpenWithOption) =>
			resolveOpenWithOptionLabel(option, i18n?.language, (key) =>
				t(`files:${key}`),
			),
		[i18n?.language, t],
	);
	const activeWopiSessionFactory = useCallback(() => {
		if (!activeOption || activeOption.mode !== "wopi" || !wopiSessionFactory) {
			return Promise.reject(new Error("wopi session factory unavailable"));
		}

		return wopiSessionFactory(activeOption.key);
	}, [activeOption, wopiSessionFactory]);
	const activeArchivePreviewFactory = useActiveArchivePreviewFactory({
		activeOption,
		archivePreviewFactory,
		open,
	});
	const usesInnerScroll = optionUsesInnerScroll(activeOption);
	const fillsViewportHeight = optionFillsViewportHeight(activeOption);
	const dialogContentClassName = getDialogContentClassName({
		fillsViewportHeight,
		isExpanded,
		showOpenMethodChooser,
	});

	return (
		<>
			<Dialog
				open={open}
				onOpenChange={handleDialogOpenChange}
				onOpenChangeComplete={onOpenChangeComplete}
			>
				<DialogContent
					animated={showOpenMethodChooser ? true : isDialogAnimationEnabled}
					keepMounted
					showCloseButton={false}
					className={dialogContentClassName}
				>
					{showOpenMethodChooser ? (
						<FilePreviewChooserContent
							file={file}
							activeMode={activeMode}
							allOptions={allOptions}
							visibleOptions={visibleOptions}
							hiddenOptions={hiddenOptions}
							showAllOpenMethods={showAllOpenMethods}
							getOptionLabel={getOptionLabel}
							onClose={onClose}
							onSelect={handleOpenMethodSelect}
							onShowAllOpenMethods={handleShowAllOpenMethods}
						/>
					) : (
						<FilePreviewPanelContent
							file={file}
							activeOption={activeOption}
							profile={profile}
							previewAppsLoaded={previewAppsLoaded}
							downloadPath={resolvedDownloadPath}
							imagePreviewPath={resolvedImagePreviewPath}
							thumbnailPath={resolvedThumbnailPath}
							getOptionLabel={getOptionLabel}
							previewLinkFactory={previewLinkFactory}
							archivePreviewFactory={activeArchivePreviewFactory}
							loadMusicBackendMetadata={resolvedLoadMusicBackendMetadata}
							mediaStreamLinkFactory={mediaStreamLinkFactory}
							createWopiSession={
								wopiSessionFactory ? activeWopiSessionFactory : null
							}
							onFileUpdated={onFileUpdated}
							onDirtyChange={handleDirtyChange}
							editable={editable}
							allOptionsCount={allOptions.length}
							usesInnerScroll={usesInnerScroll}
							fillsViewportHeight={fillsViewportHeight}
							isExpanded={isExpanded}
							isDirty={isDirty}
							onChooseOpenMethod={handleOpenMethodPickerOpen}
							onToggleExpand={handleExpandToggle}
							onClose={closeWithGuard}
						/>
					)}
				</DialogContent>
			</Dialog>
			<UnsavedChangesGuard
				open={confirmOpen}
				onOpenChange={handleConfirmOpenChange}
				onConfirm={handleDiscardChanges}
			/>
		</>
	);
}
