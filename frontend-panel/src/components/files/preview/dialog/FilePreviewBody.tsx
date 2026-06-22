import { lazy, Suspense } from "react";
import { useTranslation } from "react-i18next";
import type { ResourcePath } from "@/lib/resourceRequest";
import { normalizeTablePreviewDelimiter } from "@/lib/tablePreview";
import type {
	ArchiveFilenameEncoding,
	ArchivePreviewManifest,
	FileInfo,
	FileListItem,
} from "@/types/api";
import type { detectFilePreviewProfile } from "../capabilities/file-capabilities";
import type { OpenWithOption } from "../capabilities/types";
import type { FilePreviewResources } from "../resources/filePreviewResources";
import { PreviewLoadingState } from "../shared/PreviewLoadingState";
import { PreviewUnavailable } from "../shared/PreviewUnavailable";
import { UrlTemplatePreview } from "../viewers/external/UrlTemplatePreview";
import { BlobImagePreview } from "../viewers/image/BlobImagePreview";
import { VideoPreview } from "../viewers/video/VideoPreview";
import { WopiPreview } from "../viewers/wopi/WopiPreview";
import type { WopiSessionResource } from "../viewers/wopi/wopiSessionResource";

const PdfPreview = lazy(async () => {
	const module = await import("../viewers/pdf/PdfPreview");
	return { default: module.PdfPreview };
});

const MarkdownPreview = lazy(async () => {
	const module = await import("../viewers/text/MarkdownPreview");
	return { default: module.MarkdownPreview };
});

const CsvTablePreview = lazy(async () => {
	const module = await import("../viewers/text/CsvTablePreview");
	return { default: module.CsvTablePreview };
});

const JsonPreview = lazy(async () => {
	const module = await import("../viewers/text/JsonPreview");
	return { default: module.JsonPreview };
});

const XmlPreview = lazy(async () => {
	const module = await import("../viewers/text/XmlPreview");
	return { default: module.XmlPreview };
});

const TextCodePreview = lazy(async () => {
	const module = await import("../viewers/text/TextCodePreview");
	return { default: module.TextCodePreview };
});

const ArchivePreview = lazy(async () => {
	const module = await import("../viewers/archive/ArchivePreview");
	return { default: module.ArchivePreview };
});

type PreviewProfile = ReturnType<typeof detectFilePreviewProfile>;

interface FilePreviewBodyProps {
	file: FileInfo | FileListItem;
	activeOption: OpenWithOption | null;
	profile: PreviewProfile | null;
	previewAppsLoaded: boolean;
	contentResource: ResourcePath | null;
	resources: FilePreviewResources;
	getOptionLabel: (option: OpenWithOption) => string;
	archiveManifestLoader?: (options?: {
		signal?: AbortSignal;
		filenameEncoding?: ArchiveFilenameEncoding;
	}) => Promise<ArchivePreviewManifest>;
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
	contentResource,
	resources,
	getOptionLabel,
	archiveManifestLoader,
	wopiSessionResource,
	onFileUpdated,
	onDirtyChange,
	editable,
	formattedCategory,
	isExpanded,
}: FilePreviewBodyProps) {
	const { t } = useTranslation(["files"]);
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
		if (!contentResource) return previewLoadingState;
		return (
			<Suspense fallback={previewLoadingState}>
				<PdfPreview resource={contentResource} fileName={file.name} />
			</Suspense>
		);
	}

	if (activeOption.mode === "image") {
		return (
			<BlobImagePreview
				file={file}
				fillContainer={isExpanded}
				resource={contentResource}
			/>
		);
	}

	if (activeOption.mode === "video") {
		return (
			<VideoPreview
				file={file}
				resource={contentResource}
				createMediaStreamSession={resources.actions?.createMediaStreamSession}
			/>
		);
	}

	if (activeOption.mode === "url_template") {
		return (
			<UrlTemplatePreview
				file={file}
				downloadPath={resources.paths.download}
				label={getOptionLabel(activeOption)}
				optionKey={activeOption.key}
				rawConfig={activeOption.config ?? null}
				createExternalPreviewLink={resources.actions?.createExternalPreviewLink}
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
		if (!contentResource) return previewLoadingState;
		return (
			<Suspense fallback={previewLoadingState}>
				<MarkdownPreview resource={contentResource} />
			</Suspense>
		);
	}

	if (activeOption.mode === "table") {
		if (!contentResource) return previewLoadingState;
		const delimiter = normalizeTablePreviewDelimiter(
			activeOption.config?.delimiter,
		);

		return (
			<Suspense fallback={previewLoadingState}>
				<CsvTablePreview resource={contentResource} delimiter={delimiter} />
			</Suspense>
		);
	}

	if (activeOption.mode === "formatted") {
		if (!contentResource) return previewLoadingState;
		if (formattedCategory === "xml") {
			return (
				<Suspense fallback={previewLoadingState}>
					<XmlPreview resource={contentResource} mode="formatted" />
				</Suspense>
			);
		}

		return (
			<Suspense fallback={previewLoadingState}>
				<JsonPreview resource={contentResource} />
			</Suspense>
		);
	}

	if (activeOption.mode === "code") {
		if (!contentResource) return previewLoadingState;
		return (
			<Suspense fallback={previewLoadingState}>
				<TextCodePreview
					file={file}
					modeLabel={getOptionLabel(activeOption)}
					resource={contentResource}
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
				<ArchivePreview loadManifest={archiveManifestLoader} />
			</Suspense>
		);
	}

	return <PreviewUnavailable />;
}
