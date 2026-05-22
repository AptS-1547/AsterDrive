import { useTranslation } from "react-i18next";
import type { MusicPlayerTrack } from "@/stores/musicPlayerStore";
import type {
	ArchiveFilenameEncoding,
	ArchivePreviewManifest,
	FileInfo,
	FileListItem,
	PreviewLinkInfo,
	ShareStreamSessionInfo,
	WopiLaunchSession,
} from "@/types/api";
import { FilePreviewBody } from "./FilePreviewBody";
import { FilePreviewMethodChooser } from "./FilePreviewMethodChooser";
import { FilePreviewPanel } from "./FilePreviewPanel";
import { getFileExtension } from "./file-capabilities";
import type { FilePreviewProfile, OpenWithMode, OpenWithOption } from "./types";

export function FilePreviewChooserContent({
	file,
	activeMode,
	allOptions,
	visibleOptions,
	hiddenOptions,
	showAllOpenMethods,
	getOptionLabel,
	onClose,
	onSelect,
	onShowAllOpenMethods,
}: {
	file: FileInfo | FileListItem;
	activeMode: OpenWithMode | null;
	allOptions: OpenWithOption[];
	visibleOptions: OpenWithOption[];
	hiddenOptions: OpenWithOption[];
	showAllOpenMethods: boolean;
	getOptionLabel: (option: OpenWithOption) => string;
	onClose: () => void;
	onSelect: (mode: OpenWithMode) => void;
	onShowAllOpenMethods: () => void;
}) {
	const { t } = useTranslation(["core", "files"]);

	return (
		<FilePreviewMethodChooser
			file={file}
			activeMode={activeMode}
			allOptions={allOptions}
			visibleOptions={visibleOptions}
			hiddenOptions={hiddenOptions}
			showAllOpenMethods={showAllOpenMethods}
			getOptionLabel={getOptionLabel}
			onClose={onClose}
			onSelect={onSelect}
			onShowAllOpenMethods={onShowAllOpenMethods}
			chooseOpenMethodLabel={t("files:choose_open_method")}
			closeLabel={t("core:close")}
			moreOpenMethodsLabel={t("files:more_open_methods")}
		/>
	);
}

export function FilePreviewPanelContent({
	file,
	activeOption,
	profile,
	previewAppsLoaded,
	downloadPath,
	imagePreviewPath,
	thumbnailPath,
	getOptionLabel,
	previewLinkFactory,
	archivePreviewFactory,
	loadMusicBackendMetadata,
	mediaStreamLinkFactory,
	createWopiSession,
	onFileUpdated,
	onDirtyChange,
	editable,
	isExpanded,
	allOptionsCount,
	usesInnerScroll,
	fillsViewportHeight,
	isDirty,
	onChooseOpenMethod,
	onToggleExpand,
	onClose,
}: {
	file: FileInfo | FileListItem;
	activeOption: OpenWithOption | null;
	profile: FilePreviewProfile | null;
	previewAppsLoaded: boolean;
	downloadPath: string;
	imagePreviewPath?: string;
	thumbnailPath?: string;
	getOptionLabel: (option: OpenWithOption) => string;
	previewLinkFactory?: () => Promise<PreviewLinkInfo>;
	archivePreviewFactory?: (options?: {
		signal?: AbortSignal;
		filenameEncoding?: ArchiveFilenameEncoding;
	}) => Promise<ArchivePreviewManifest>;
	loadMusicBackendMetadata?: MusicPlayerTrack["loadBackendMetadata"];
	mediaStreamLinkFactory?: () => Promise<ShareStreamSessionInfo>;
	createWopiSession?: (() => Promise<WopiLaunchSession>) | null;
	onFileUpdated?: () => void;
	onDirtyChange: (dirty: boolean) => void;
	editable: boolean;
	isExpanded: boolean;
	allOptionsCount: number;
	usesInnerScroll: boolean;
	fillsViewportHeight: boolean;
	isDirty: boolean;
	onChooseOpenMethod: () => void;
	onToggleExpand: () => void;
	onClose: () => void;
}) {
	const { t } = useTranslation(["core", "files"]);

	return (
		<FilePreviewPanel
			file={file}
			body={
				<FilePreviewBody
					file={file}
					activeOption={activeOption}
					profile={profile}
					previewAppsLoaded={previewAppsLoaded}
					downloadPath={downloadPath}
					imagePreviewPath={imagePreviewPath}
					thumbnailPath={thumbnailPath}
					getOptionLabel={getOptionLabel}
					previewLinkFactory={previewLinkFactory}
					archivePreviewFactory={archivePreviewFactory}
					loadMusicBackendMetadata={loadMusicBackendMetadata}
					mediaStreamLinkFactory={mediaStreamLinkFactory}
					createWopiSession={createWopiSession}
					onFileUpdated={onFileUpdated}
					onDirtyChange={onDirtyChange}
					editable={editable}
					isExpanded={isExpanded}
					formattedCategory={
						profile?.category === "xml" || getFileExtension(file) === "xml"
							? "xml"
							: "json"
					}
				/>
			}
			allOptionsCount={allOptionsCount}
			usesInnerScroll={usesInnerScroll}
			fillsViewportHeight={fillsViewportHeight}
			isExpanded={isExpanded}
			isDirty={isDirty}
			onChooseOpenMethod={onChooseOpenMethod}
			onToggleExpand={onToggleExpand}
			onClose={onClose}
			chooseOpenMethodLabel={t("files:choose_open_method")}
			enterFullscreenLabel={t("files:preview_enter_fullscreen")}
			exitFullscreenLabel={t("files:preview_exit_fullscreen")}
			closeLabel={t("core:close")}
		/>
	);
}
