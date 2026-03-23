import type { IconName } from "@/components/ui/icon";
import type {
	FilePreviewProfile,
	FileTypeInfo,
	OpenWithOption,
	PreviewableFileLike,
} from "./types";

const TEXT_EXTENSIONS = new Set([
	"txt",
	"md",
	"markdown",
	"log",
	"env",
	"ini",
	"conf",
	"cfg",
	"toml",
	"yaml",
	"yml",
	"json",
	"xml",
	"csv",
	"tsv",
	"js",
	"jsx",
	"ts",
	"tsx",
	"mjs",
	"cjs",
	"py",
	"rs",
	"go",
	"java",
	"kt",
	"swift",
	"c",
	"h",
	"cpp",
	"hpp",
	"cs",
	"php",
	"rb",
	"lua",
	"r",
	"sh",
	"bash",
	"zsh",
	"fish",
	"sql",
	"html",
	"htm",
	"css",
	"scss",
	"less",
	"svg",
]);

const SPECIAL_TEXT_FILENAMES = new Map<string, string>([
	["dockerfile", "dockerfile"],
	["makefile", "plaintext"],
	[".gitignore", "plaintext"],
	[".env", "plaintext"],
]);

const LANGUAGE_BY_EXTENSION: Record<string, string> = {
	js: "javascript",
	jsx: "javascript",
	ts: "typescript",
	tsx: "typescript",
	mjs: "javascript",
	cjs: "javascript",
	json: "json",
	md: "markdown",
	markdown: "markdown",
	py: "python",
	rs: "rust",
	go: "go",
	html: "html",
	htm: "html",
	css: "css",
	scss: "scss",
	less: "less",
	xml: "xml",
	yaml: "yaml",
	yml: "yaml",
	toml: "toml",
	sh: "shell",
	bash: "shell",
	zsh: "shell",
	fish: "shell",
	sql: "sql",
	java: "java",
	kt: "kotlin",
	swift: "swift",
	c: "c",
	h: "c",
	cpp: "cpp",
	hpp: "cpp",
	cs: "csharp",
	rb: "ruby",
	php: "php",
	lua: "lua",
	r: "r",
	csv: "plaintext",
	tsv: "plaintext",
	log: "plaintext",
	env: "plaintext",
	ini: "ini",
	conf: "plaintext",
	cfg: "plaintext",
	svg: "xml",
};

const DOCUMENT_MIME_TYPES = new Map<string, { icon: IconName; color: string }>([
	["application/pdf", { icon: "FileText", color: "text-red-500" }],
	["application/json", { icon: "BracketsCurly", color: "text-amber-500" }],
	["application/msword", { icon: "FileText", color: "text-blue-500" }],
	["application/vnd.ms-excel", { icon: "Table", color: "text-green-600" }],
	[
		"application/vnd.ms-powerpoint",
		{ icon: "Presentation", color: "text-orange-500" },
	],
]);

const PREFIX_TYPE_INFO: Array<
	[
		string,
		{ category: FileTypeInfo["category"]; icon: IconName; color: string },
	]
> = [
	[
		"application/vnd.openxmlformats-officedocument.wordprocessingml",
		{ category: "document", icon: "FileText", color: "text-blue-500" },
	],
	[
		"application/vnd.openxmlformats-officedocument.spreadsheetml",
		{ category: "spreadsheet", icon: "Table", color: "text-green-600" },
	],
	[
		"application/vnd.openxmlformats-officedocument.presentationml",
		{
			category: "presentation",
			icon: "Presentation",
			color: "text-orange-500",
		},
	],
	["image/", { category: "image", icon: "FileText", color: "text-sky-500" }],
	[
		"video/",
		{ category: "video", icon: "FileVideo", color: "text-purple-500" },
	],
	["audio/", { category: "audio", icon: "FileAudio", color: "text-pink-500" }],
	["text/", { category: "text", icon: "FileCode", color: "text-slate-500" }],
	[
		"application/zip",
		{ category: "archive", icon: "FileZip", color: "text-yellow-600" },
	],
	[
		"application/x-tar",
		{ category: "archive", icon: "FileZip", color: "text-yellow-600" },
	],
	[
		"application/gzip",
		{ category: "archive", icon: "FileZip", color: "text-yellow-600" },
	],
	[
		"application/x-rar",
		{ category: "archive", icon: "FileZip", color: "text-yellow-600" },
	],
	[
		"application/x-7z",
		{ category: "archive", icon: "FileZip", color: "text-yellow-600" },
	],
];

const DEFAULT_TYPE_INFO: FileTypeInfo = {
	category: "unknown",
	icon: "File",
	color: "text-muted-foreground",
};

export const OPEN_WITH_OPTIONS: Record<string, OpenWithOption[]> = {
	image: [{ mode: "image", labelKey: "open_with_image", icon: "Eye" }],
	video: [{ mode: "video", labelKey: "open_with_video", icon: "Monitor" }],
	audio: [{ mode: "audio", labelKey: "open_with_audio", icon: "FileAudio" }],
	pdf: [{ mode: "pdf", labelKey: "open_with_pdf", icon: "FileText" }],
	markdown: [
		{ mode: "markdown", labelKey: "open_with_markdown", icon: "Eye" },
		{ mode: "code", labelKey: "open_with_code", icon: "FileCode" },
	],
	csv: [
		{ mode: "table", labelKey: "open_with_table", icon: "Table" },
		{ mode: "code", labelKey: "open_with_code", icon: "FileCode" },
	],
	tsv: [
		{ mode: "table", labelKey: "open_with_table", icon: "Table" },
		{ mode: "code", labelKey: "open_with_code", icon: "FileCode" },
	],
	json: [
		{
			mode: "formatted",
			labelKey: "open_with_formatted",
			icon: "BracketsCurly",
		},
		{ mode: "code", labelKey: "open_with_code", icon: "FileCode" },
	],
	xml: [
		{
			mode: "formatted",
			labelKey: "open_with_formatted",
			icon: "BracketsCurly",
		},
		{ mode: "code", labelKey: "open_with_code", icon: "FileCode" },
	],
	text: [{ mode: "code", labelKey: "open_with_code", icon: "FileCode" }],
};

function getExtension(name: string) {
	const trimmed = name.trim();
	const lower = trimmed.toLowerCase();
	const special = SPECIAL_TEXT_FILENAMES.get(lower);
	if (special) return { ext: lower, specialLanguage: special };
	const dot = lower.lastIndexOf(".");
	if (dot < 0) return { ext: "", specialLanguage: null };
	return { ext: lower.slice(dot + 1), specialLanguage: null };
}

export function getFileExtension(file: PreviewableFileLike) {
	return getExtension(file.name).ext;
}

export function getEditorLanguage(file: PreviewableFileLike): string {
	const { ext, specialLanguage } = getExtension(file.name);
	if (specialLanguage) return specialLanguage;
	return LANGUAGE_BY_EXTENSION[ext] ?? "plaintext";
}

export function getFileTypeInfo(file: PreviewableFileLike): FileTypeInfo {
	const exact = DOCUMENT_MIME_TYPES.get(file.mime_type);
	if (exact) {
		if (file.mime_type === "application/pdf") {
			return { category: "pdf", ...exact };
		}
		if (file.mime_type === "application/json") {
			return { category: "json", ...exact };
		}
		return { category: "document", ...exact };
	}

	for (const [prefix, info] of PREFIX_TYPE_INFO) {
		if (file.mime_type.startsWith(prefix)) return info;
	}

	const { ext } = getExtension(file.name);
	if (ext === "md" || ext === "markdown") {
		return { category: "markdown", icon: "Scroll", color: "text-sky-500" };
	}
	if (ext === "csv" || ext === "tsv") {
		return { category: ext, icon: "Table", color: "text-green-600" };
	}
	if (ext === "svg") {
		return { category: "image", icon: "FileText", color: "text-sky-500" };
	}
	if (ext === "xml") {
		return { category: "xml", icon: "BracketsCurly", color: "text-orange-500" };
	}
	if (ext === "json") {
		return { category: "json", icon: "BracketsCurly", color: "text-amber-500" };
	}
	if (TEXT_EXTENSIONS.has(ext)) {
		return { category: "text", icon: "FileCode", color: "text-slate-500" };
	}
	return DEFAULT_TYPE_INFO;
}

export function detectFilePreviewProfile(
	file: PreviewableFileLike,
): FilePreviewProfile {
	const typeInfo = getFileTypeInfo(file);
	const { ext } = getExtension(file.name);

	if (typeInfo.category === "image") {
		return {
			category: "image",
			isBlobPreview: true,
			isTextBased: false,
			isEditableText: false,
			defaultMode: "image",
			options: OPEN_WITH_OPTIONS.image,
		};
	}
	if (typeInfo.category === "video") {
		return {
			category: "video",
			isBlobPreview: true,
			isTextBased: false,
			isEditableText: false,
			defaultMode: "video",
			options: OPEN_WITH_OPTIONS.video,
		};
	}
	if (typeInfo.category === "audio") {
		return {
			category: "audio",
			isBlobPreview: true,
			isTextBased: false,
			isEditableText: false,
			defaultMode: "audio",
			options: OPEN_WITH_OPTIONS.audio,
		};
	}
	if (typeInfo.category === "pdf") {
		return {
			category: "pdf",
			isBlobPreview: true,
			isTextBased: false,
			isEditableText: false,
			defaultMode: "pdf",
			options: OPEN_WITH_OPTIONS.pdf,
		};
	}
	if (typeInfo.category === "markdown") {
		return {
			category: "markdown",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: "markdown",
			options: OPEN_WITH_OPTIONS.markdown,
		};
	}
	if (typeInfo.category === "csv") {
		return {
			category: "csv",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: "table",
			options: OPEN_WITH_OPTIONS.csv,
		};
	}
	if (typeInfo.category === "tsv") {
		return {
			category: "tsv",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: "table",
			options: OPEN_WITH_OPTIONS.tsv,
		};
	}
	if (typeInfo.category === "json") {
		return {
			category: "json",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: "formatted",
			options: OPEN_WITH_OPTIONS.json,
		};
	}
	if (typeInfo.category === "xml") {
		return {
			category: "xml",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: "formatted",
			options: OPEN_WITH_OPTIONS.xml,
		};
	}

	const isKnownText =
		typeInfo.category === "text" ||
		TEXT_EXTENSIONS.has(ext) ||
		file.mime_type === "application/javascript" ||
		file.mime_type === "application/x-sh" ||
		file.mime_type === "application/x-yaml" ||
		file.mime_type === "application/toml";

	if (isKnownText) {
		return {
			category: "text",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: "code",
			options: OPEN_WITH_OPTIONS.text,
		};
	}

	return {
		category: typeInfo.category,
		isBlobPreview: false,
		isTextBased: false,
		isEditableText: false,
		defaultMode: null,
		options: [],
	};
}

export function getAvailableOpenWithOptions(file: PreviewableFileLike) {
	return detectFilePreviewProfile(file).options;
}

export function getDefaultOpenWith(file: PreviewableFileLike) {
	return detectFilePreviewProfile(file).defaultMode;
}

export function isEditableTextFile(file: PreviewableFileLike) {
	return detectFilePreviewProfile(file).isEditableText;
}
