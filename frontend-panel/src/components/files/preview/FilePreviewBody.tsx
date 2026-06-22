import { lazy, Suspense } from "react";
import { useTranslation } from "react-i18next";
import { type ResourcePath, resourceRequestPath } from "@/lib/resourceRequest";
import { normalizeTablePreviewDelimiter } from "@/lib/tablePreview";
import type { MusicPlayerTrack } from "@/stores/musicPlayerStore";
import type {
	ArchiveFilenameEncoding,
	ArchivePreviewManifest,
	FileInfo,
	FileListItem,
	PreviewLinkInfo,
	ShareStreamSessionInfo,
} from "@/types/api";
import { BlobImagePreview } from "./BlobImagePreview";
import type { detectFilePreviewProfile } from "./file-capabilities";
import { MusicPreview } from "./MusicPreview";
import { PreviewLoadingState } from "./PreviewLoadingState";
import { PreviewUnavailable } from "./PreviewUnavailable";
import type { OpenWithOption } from "./types";
import { UrlTemplatePreview } from "./UrlTemplatePreview";
import { VideoPreview } from "./VideoPreview";
import { WopiPreview } from "./WopiPreview";
import type { WopiSessionResource } from "./wopiSessionResource";

const PdfPreview = lazy(async () => {
	const module = await import("./PdfPreview");
	return { default: module.PdfPreview };
});

const MarkdownPreview = lazy(async () => {
	const module = await import("./MarkdownPreview");
	return { default: module.MarkdownPreview };
});

const CsvTablePreview = lazy(async () => {
	const module = await import("./CsvTablePreview");
	return { default: module.CsvTablePreview };
});

const JsonPreview = lazy(async () => {
	const module = await import("./JsonPreview");
	return { default: module.JsonPreview };
});

const XmlPreview = lazy(async () => {
	const module = await import("./XmlPreview");
	return { default: module.XmlPreview };
});

const TextCodePreview = lazy(async () => {
	const module = await import("./TextCodePreview");
	return { default: module.TextCodePreview };
});

const ArchivePreview = lazy(async () => {
	const module = await import("./ArchivePreview");
	return { default: module.ArchivePreview };
});

type PreviewProfile = ReturnType<typeof detectFilePreviewProfile>;

interface FilePreviewBodyProps {
	file: FileInfo | FileListItem;
	activeOption: OpenWithOption | null;
	profile: PreviewProfile | null;
	previewAppsLoaded: boolean;
	contentPreviewPath: ResourcePath | null;
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
	wopiSessionResource?: WopiSessionResource | null;
	onFileUpdated?: () => void;
	onDirtyChange: (dirty: boolean) => void;
	editable: boolean;
	formattedCategory: "json" | "xml";
	isExpanded: boolean;
}

export function FilePreviewBody({
	file,
	activeOption,
	profile,
	previewAppsLoaded,
	contentPreviewPath,
	downloadPath,
	imagePreviewPath,
	thumbnailPath,
	getOptionLabel,
	previewLinkFactory,
	archivePreviewFactory,
	loadMusicBackendMetadata,
	mediaStreamLinkFactory,
	wopiSessionResource,
	onFileUpdated,
	onDirtyChange,
	editable,
	formattedCategory,
	isExpanded,
}: FilePreviewBodyProps) {
	const { t } = useTranslation(["files"]);
	const contentPreviewRequestPath = contentPreviewPath
		? resourceRequestPath(contentPreviewPath)
		: null;
	const previewLoadingState = (
		<PreviewLoadingState
			text={t("files:loading_preview")}
			className="h-full min-h-[16rem]"
		/>
	);

	if (!previewAppsLoaded) {
		return previewLoadingState;
	}
	if (!profile || !activeOption) {
		return <PreviewUnavailable />;
	}

	if (activeOption.mode === "pdf") {
		if (!contentPreviewPath) return previewLoadingState;
		return (
			<Suspense fallback={previewLoadingState}>
				<PdfPreview path={contentPreviewPath} fileName={file.name} />
			</Suspense>
		);
	}

	if (activeOption.mode === "image") {
		return (
			<BlobImagePreview
				file={file}
				fillContainer={isExpanded}
				path={contentPreviewPath}
				fallbackPath={imagePreviewPath}
			/>
		);
	}

	if (activeOption.mode === "audio") {
		return (
			<MusicPreview
				file={file}
				loadBackendMetadata={loadMusicBackendMetadata}
				path={contentPreviewRequestPath}
				thumbnailPath={thumbnailPath}
				mediaStreamLinkFactory={mediaStreamLinkFactory}
			/>
		);
	}

	if (activeOption.mode === "video") {
		return (
			<VideoPreview
				file={file}
				path={contentPreviewRequestPath}
				mediaStreamLinkFactory={mediaStreamLinkFactory}
			/>
		);
	}

	if (activeOption.mode === "url_template") {
		return (
			<UrlTemplatePreview
				file={file}
				downloadPath={downloadPath}
				label={getOptionLabel(activeOption)}
				optionKey={activeOption.key}
				rawConfig={activeOption.config ?? null}
				createPreviewLink={previewLinkFactory}
			/>
		);
	}

	if (activeOption.mode === "wopi") {
		if (!wopiSessionResource) {
			return <PreviewUnavailable />;
		}
		return (
			<WopiPreview
				label={getOptionLabel(activeOption)}
				rawConfig={activeOption.config ?? null}
				sessionResource={wopiSessionResource}
			/>
		);
	}

	if (activeOption.mode === "markdown") {
		if (!contentPreviewPath) return previewLoadingState;
		return (
			<Suspense fallback={previewLoadingState}>
				<MarkdownPreview path={contentPreviewPath} />
			</Suspense>
		);
	}

	if (activeOption.mode === "table") {
		if (!contentPreviewPath) return previewLoadingState;
		const delimiter = normalizeTablePreviewDelimiter(
			activeOption.config?.delimiter,
		);

		return (
			<Suspense fallback={previewLoadingState}>
				<CsvTablePreview path={contentPreviewPath} delimiter={delimiter} />
			</Suspense>
		);
	}

	if (activeOption.mode === "formatted") {
		if (!contentPreviewPath) return previewLoadingState;
		if (formattedCategory === "xml") {
			return (
				<Suspense fallback={previewLoadingState}>
					<XmlPreview path={contentPreviewPath} mode="formatted" />
				</Suspense>
			);
		}

		return (
			<Suspense fallback={previewLoadingState}>
				<JsonPreview path={contentPreviewPath} />
			</Suspense>
		);
	}

	if (activeOption.mode === "code") {
		if (!contentPreviewPath) return previewLoadingState;
		return (
			<Suspense fallback={previewLoadingState}>
				<TextCodePreview
					file={file}
					modeLabel={getOptionLabel(activeOption)}
					path={contentPreviewPath}
					onFileUpdated={onFileUpdated}
					onDirtyChange={onDirtyChange}
					editable={editable}
				/>
			</Suspense>
		);
	}

	if (activeOption.mode === "archive") {
		return (
			<Suspense fallback={previewLoadingState}>
				<ArchivePreview loadManifest={archivePreviewFactory} />
			</Suspense>
		);
	}

	return <PreviewUnavailable />;
}
