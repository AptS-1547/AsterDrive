import type { IconName } from "@/components/ui/icon";

export type FileCategory =
	| "image"
	| "video"
	| "audio"
	| "pdf"
	| "markdown"
	| "csv"
	| "tsv"
	| "json"
	| "xml"
	| "text"
	| "archive"
	| "document"
	| "spreadsheet"
	| "presentation"
	| "unknown";

export type OpenWithMode =
	| "image"
	| "video"
	| "videoBrowser"
	| "audio"
	| "pdf"
	| "markdown"
	| "table"
	| "structured"
	| "formatted"
	| "code";

export interface OpenWithOption {
	mode: OpenWithMode;
	labelKey: string;
	label?: string;
	icon: IconName;
}

export interface FileTypeInfo {
	category: FileCategory;
	icon: IconName;
	color: string;
}

export interface FilePreviewProfile {
	category: FileCategory;
	isBlobPreview: boolean;
	isTextBased: boolean;
	isEditableText: boolean;
	defaultMode: OpenWithMode | null;
	options: OpenWithOption[];
}

export interface PreviewableFileLike {
	name: string;
	mime_type: string;
	size?: number;
}
