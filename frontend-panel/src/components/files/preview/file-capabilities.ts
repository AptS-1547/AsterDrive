import {
	getBuiltinPreviewAppIconUrl,
	PREVIEW_APP_ICON_URLS,
} from "@/components/common/previewAppIconUrls";
import type { IconName } from "@/components/ui/icon";
import {
	BUILTIN_TABLE_PREVIEW_APP_KEY,
	DEFAULT_TABLE_PREVIEW_DELIMITER,
} from "@/lib/tablePreview";
import type {
	PreviewAppProvider,
	PublicPreviewAppDefinition,
	PublicPreviewAppsConfig,
} from "@/types/api";
import type {
	FilePreviewProfile,
	FileTypeInfo,
	OpenWithOption,
	PreviewableFileLike,
} from "./types";

const TEXT_EXTENSIONS = new Set([
	// Plain text & markup
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
	"rst",
	"tex",
	"bib",
	"adoc",
	// Web
	"html",
	"htm",
	"css",
	"scss",
	"less",
	"svg",
	"vue",
	"svelte",
	"astro",
	// JavaScript / TypeScript
	"js",
	"jsx",
	"ts",
	"tsx",
	"mjs",
	"cjs",
	"coffee",
	// Systems languages
	"c",
	"h",
	"cpp",
	"hpp",
	"cs",
	"rs",
	"go",
	"dart",
	"zig",
	"nim",
	"d",
	"asm",
	// JVM
	"java",
	"kt",
	"kts",
	"scala",
	"sbt",
	"groovy",
	"clj",
	"cljs",
	// Scripting
	"py",
	"rb",
	"php",
	"pl",
	"pm",
	"lua",
	"r",
	"jl",
	"vim",
	"el",
	// Shell
	"sh",
	"bash",
	"zsh",
	"fish",
	"ps1",
	"psm1",
	"bat",
	"cmd",
	// Functional
	"hs",
	"ex",
	"exs",
	"erl",
	// Query / data
	"sql",
	"graphql",
	"gql",
	"proto",
	"prisma",
	// IaC / config
	"tf",
	"tfvars",
	"hcl",
	"properties",
	"cmake",
	"gradle",
	// Hardware
	"v",
	"sv",
	"vhd",
	"vhdl",
	// Web3
	"sol",
	// VCS
	"diff",
	"patch",
]);

const IMAGE_EXTENSIONS = new Set([
	"avif",
	"bmp",
	"gif",
	"heic",
	"heif",
	"ico",
	"jfif",
	"jpe",
	"jpeg",
	"jpg",
	"png",
	"svg",
	"tif",
	"tiff",
	"webp",
]);

const SPECIAL_TEXT_FILENAMES = new Map<string, string>([
	["dockerfile", "dockerfile"],
	["makefile", "plaintext"],
	[".gitignore", "plaintext"],
	[".env", "plaintext"],
	[".dockerignore", "plaintext"],
	[".editorconfig", "ini"],
	[".gitattributes", "plaintext"],
	[".gitmodules", "plaintext"],
	[".npmrc", "ini"],
	[".npmignore", "plaintext"],
	["jenkinsfile", "groovy"],
	["vagrantfile", "ruby"],
	["gemfile", "ruby"],
	["rakefile", "ruby"],
	["procfile", "plaintext"],
	[".mailmap", "plaintext"],
]);

const LANGUAGE_BY_EXTENSION: Record<string, string> = {
	// Web
	js: "javascript",
	jsx: "javascript",
	ts: "typescript",
	tsx: "typescript",
	mjs: "javascript",
	cjs: "javascript",
	coffee: "coffeescript",
	html: "html",
	htm: "html",
	css: "css",
	scss: "scss",
	less: "less",
	svg: "xml",
	vue: "html",
	svelte: "html",
	astro: "html",
	// Data / markup
	json: "json",
	xml: "xml",
	yaml: "yaml",
	yml: "yaml",
	toml: "toml",
	md: "markdown",
	markdown: "markdown",
	rst: "restructuredtext",
	tex: "plaintext",
	bib: "plaintext",
	adoc: "plaintext",
	csv: "plaintext",
	tsv: "plaintext",
	// Systems
	c: "c",
	h: "c",
	cpp: "cpp",
	hpp: "cpp",
	cs: "csharp",
	rs: "rust",
	go: "go",
	dart: "dart",
	zig: "plaintext",
	nim: "plaintext",
	d: "plaintext",
	asm: "plaintext",
	// JVM
	java: "java",
	kt: "kotlin",
	kts: "kotlin",
	scala: "scala",
	sbt: "scala",
	groovy: "groovy",
	clj: "clojure",
	cljs: "clojure",
	// Scripting
	py: "python",
	rb: "ruby",
	php: "php",
	pl: "perl",
	pm: "perl",
	lua: "lua",
	r: "r",
	jl: "julia",
	vim: "plaintext",
	el: "plaintext",
	// Shell
	sh: "shell",
	bash: "shell",
	zsh: "shell",
	fish: "shell",
	ps1: "powershell",
	psm1: "powershell",
	bat: "bat",
	cmd: "bat",
	swift: "swift",
	// Functional
	hs: "plaintext",
	ex: "elixir",
	exs: "elixir",
	erl: "plaintext",
	// Query / schema
	sql: "sql",
	graphql: "graphql",
	gql: "graphql",
	proto: "protobuf",
	prisma: "plaintext",
	// IaC / config
	tf: "hcl",
	tfvars: "hcl",
	hcl: "hcl",
	properties: "ini",
	cmake: "plaintext",
	gradle: "java",
	// Hardware
	v: "verilog",
	sv: "systemverilog",
	vhd: "plaintext",
	vhdl: "plaintext",
	// Web3
	sol: "sol",
	// VCS
	diff: "plaintext",
	patch: "plaintext",
	// Plain text fallbacks
	log: "plaintext",
	env: "plaintext",
	ini: "ini",
	conf: "plaintext",
	cfg: "plaintext",
	txt: "plaintext",
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
	["image/", { category: "image", icon: "FileImage", color: "text-sky-500" }],
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

const GOOGLE_VIEWER_CONFIG = {
	allowed_origins: ["https://docs.google.com"],
	mode: "iframe",
	url_template:
		"https://docs.google.com/gview?embedded=true&url={{file_preview_url}}",
} as const;

const MICROSOFT_VIEWER_CONFIG = {
	allowed_origins: ["https://view.officeapps.live.com"],
	mode: "iframe",
	url_template:
		"https://view.officeapps.live.com/op/embed.aspx?src={{file_preview_url}}",
} as const;

export const BUILTIN_PREVIEW_OPTIONS: Record<string, OpenWithOption[]> = {
	image: [
		{
			key: "builtin.image",
			mode: "image",
			labelKey: "open_with_image",
			icon: PREVIEW_APP_ICON_URLS.image,
		},
	],
	video: [
		{
			key: "builtin.video",
			mode: "video",
			labelKey: "open_with_video",
			icon: PREVIEW_APP_ICON_URLS.video,
		},
	],
	audio: [
		{
			key: "builtin.audio",
			mode: "audio",
			labelKey: "open_with_audio",
			icon: PREVIEW_APP_ICON_URLS.audio,
		},
	],
	pdf: [
		{
			key: "builtin.pdf",
			mode: "pdf",
			labelKey: "open_with_pdf",
			icon: PREVIEW_APP_ICON_URLS.pdf,
		},
	],
	document: [
		{
			key: "builtin.office_microsoft",
			mode: "url_template",
			labelKey: "open_with_office_microsoft",
			icon: PREVIEW_APP_ICON_URLS.microsoftOnedrive,
			config: MICROSOFT_VIEWER_CONFIG,
		},
		{
			key: "builtin.office_google",
			mode: "url_template",
			labelKey: "open_with_office_google",
			icon: PREVIEW_APP_ICON_URLS.googleDrive,
			config: GOOGLE_VIEWER_CONFIG,
		},
	],
	spreadsheet: [
		{
			key: "builtin.office_microsoft",
			mode: "url_template",
			labelKey: "open_with_office_microsoft",
			icon: PREVIEW_APP_ICON_URLS.microsoftOnedrive,
			config: MICROSOFT_VIEWER_CONFIG,
		},
		{
			key: "builtin.office_google",
			mode: "url_template",
			labelKey: "open_with_office_google",
			icon: PREVIEW_APP_ICON_URLS.googleDrive,
			config: GOOGLE_VIEWER_CONFIG,
		},
	],
	presentation: [
		{
			key: "builtin.office_microsoft",
			mode: "url_template",
			labelKey: "open_with_office_microsoft",
			icon: PREVIEW_APP_ICON_URLS.microsoftOnedrive,
			config: MICROSOFT_VIEWER_CONFIG,
		},
		{
			key: "builtin.office_google",
			mode: "url_template",
			labelKey: "open_with_office_google",
			icon: PREVIEW_APP_ICON_URLS.googleDrive,
			config: GOOGLE_VIEWER_CONFIG,
		},
	],
	markdown: [
		{
			key: "builtin.markdown",
			mode: "markdown",
			labelKey: "open_with_markdown",
			icon: PREVIEW_APP_ICON_URLS.markdown,
		},
		{
			key: "builtin.code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
	csv: [
		{
			key: BUILTIN_TABLE_PREVIEW_APP_KEY,
			mode: "table",
			labelKey: "open_with_table",
			icon: PREVIEW_APP_ICON_URLS.table,
			config: { delimiter: DEFAULT_TABLE_PREVIEW_DELIMITER },
		},
		{
			key: "builtin.code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
	tsv: [
		{
			key: BUILTIN_TABLE_PREVIEW_APP_KEY,
			mode: "table",
			labelKey: "open_with_table",
			icon: PREVIEW_APP_ICON_URLS.table,
			config: { delimiter: DEFAULT_TABLE_PREVIEW_DELIMITER },
		},
		{
			key: "builtin.code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
	json: [
		{
			key: "builtin.formatted",
			mode: "formatted",
			labelKey: "open_with_formatted",
			icon: PREVIEW_APP_ICON_URLS.json,
		},
		{
			key: "builtin.code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
	xml: [
		{
			key: "builtin.formatted",
			mode: "formatted",
			labelKey: "open_with_formatted",
			icon: PREVIEW_APP_ICON_URLS.xml,
		},
		{
			key: "builtin.code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
	text: [
		{
			key: "builtin.code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
	svg: [
		{
			key: "builtin.image",
			mode: "image",
			labelKey: "open_with_image",
			icon: PREVIEW_APP_ICON_URLS.image,
		},
		{
			key: "builtin.code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
};

type ConfiguredPreviewApp = PublicPreviewAppDefinition;

function mergeOpenWithOptions(...groups: OpenWithOption[][]): OpenWithOption[] {
	const merged: OpenWithOption[] = [];

	for (const group of groups) {
		for (const option of group) {
			if (merged.some((candidate) => candidate.key === option.key)) {
				continue;
			}
			merged.push(option);
		}
	}

	return merged;
}

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

function isSvgFile(file: PreviewableFileLike) {
	const { ext } = getExtension(file.name);
	return ext === "svg" || file.mime_type === "image/svg+xml";
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

	const { ext } = getExtension(file.name);
	if (
		file.mime_type === "text/markdown" ||
		ext === "md" ||
		ext === "markdown"
	) {
		return { category: "markdown", icon: "Scroll", color: "text-sky-500" };
	}
	if (file.mime_type === "text/csv" || ext === "csv") {
		return { category: "csv", icon: "Table", color: "text-green-600" };
	}
	if (file.mime_type === "text/tab-separated-values" || ext === "tsv") {
		return { category: "tsv", icon: "Table", color: "text-green-600" };
	}
	if (
		file.mime_type === "text/xml" ||
		file.mime_type === "application/xml" ||
		ext === "xml"
	) {
		return { category: "xml", icon: "BracketsCurly", color: "text-orange-500" };
	}
	if (ext === "json") {
		return { category: "json", icon: "BracketsCurly", color: "text-amber-500" };
	}
	if (IMAGE_EXTENSIONS.has(ext)) {
		return { category: "image", icon: "FileImage", color: "text-sky-500" };
	}
	if (ext === "doc" || ext === "docx" || ext === "odt") {
		return { category: "document", icon: "FileText", color: "text-blue-500" };
	}
	if (ext === "xls" || ext === "xlsx" || ext === "ods") {
		return { category: "spreadsheet", icon: "Table", color: "text-green-600" };
	}
	if (ext === "ppt" || ext === "pptx" || ext === "odp") {
		return {
			category: "presentation",
			icon: "Presentation",
			color: "text-orange-500",
		};
	}

	for (const [prefix, info] of PREFIX_TYPE_INFO) {
		if (file.mime_type.startsWith(prefix)) return info;
	}

	if (TEXT_EXTENSIONS.has(ext)) {
		return { category: "text", icon: "FileCode", color: "text-slate-500" };
	}
	return DEFAULT_TYPE_INFO;
}

function detectBuiltinFilePreviewProfile(
	file: PreviewableFileLike,
): FilePreviewProfile {
	const typeInfo = getFileTypeInfo(file);
	const { ext } = getExtension(file.name);
	const isOpenDocument = ext === "odt" || ext === "ods" || ext === "odp";

	if (isSvgFile(file)) {
		return {
			category: "image",
			isBlobPreview: true,
			isTextBased: true,
			isEditableText: true,
			defaultMode: "builtin.image",
			options: BUILTIN_PREVIEW_OPTIONS.svg,
		};
	}

	if (typeInfo.category === "image") {
		return {
			category: "image",
			isBlobPreview: true,
			isTextBased: false,
			isEditableText: false,
			defaultMode: "builtin.image",
			options: BUILTIN_PREVIEW_OPTIONS.image,
		};
	}
	if (typeInfo.category === "video") {
		return {
			category: "video",
			isBlobPreview: true,
			isTextBased: false,
			isEditableText: false,
			defaultMode: "builtin.video",
			options: BUILTIN_PREVIEW_OPTIONS.video,
		};
	}
	if (typeInfo.category === "audio") {
		return {
			category: "audio",
			isBlobPreview: true,
			isTextBased: false,
			isEditableText: false,
			defaultMode: "builtin.audio",
			options: BUILTIN_PREVIEW_OPTIONS.audio,
		};
	}
	if (typeInfo.category === "pdf") {
		return {
			category: "pdf",
			isBlobPreview: true,
			isTextBased: false,
			isEditableText: false,
			defaultMode: "builtin.pdf",
			options: BUILTIN_PREVIEW_OPTIONS.pdf,
		};
	}
	if (typeInfo.category === "document") {
		return {
			category: "document",
			isBlobPreview: false,
			isTextBased: false,
			isEditableText: false,
			defaultMode: isOpenDocument
				? "builtin.office_google"
				: "builtin.office_microsoft",
			options: isOpenDocument
				? [BUILTIN_PREVIEW_OPTIONS.document[1]].filter(
						(
							option,
						): option is (typeof BUILTIN_PREVIEW_OPTIONS.document)[number] =>
							option !== undefined,
					)
				: BUILTIN_PREVIEW_OPTIONS.document,
		};
	}
	if (typeInfo.category === "spreadsheet") {
		return {
			category: "spreadsheet",
			isBlobPreview: false,
			isTextBased: false,
			isEditableText: false,
			defaultMode: isOpenDocument
				? "builtin.office_google"
				: "builtin.office_microsoft",
			options: isOpenDocument
				? [BUILTIN_PREVIEW_OPTIONS.spreadsheet[1]].filter(
						(
							option,
						): option is (typeof BUILTIN_PREVIEW_OPTIONS.spreadsheet)[number] =>
							option !== undefined,
					)
				: BUILTIN_PREVIEW_OPTIONS.spreadsheet,
		};
	}
	if (typeInfo.category === "presentation") {
		return {
			category: "presentation",
			isBlobPreview: false,
			isTextBased: false,
			isEditableText: false,
			defaultMode: isOpenDocument
				? "builtin.office_google"
				: "builtin.office_microsoft",
			options: isOpenDocument
				? [BUILTIN_PREVIEW_OPTIONS.presentation[1]].filter(
						(
							option,
						): option is (typeof BUILTIN_PREVIEW_OPTIONS.presentation)[number] =>
							option !== undefined,
					)
				: BUILTIN_PREVIEW_OPTIONS.presentation,
		};
	}
	if (typeInfo.category === "markdown") {
		return {
			category: "markdown",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: "builtin.markdown",
			options: BUILTIN_PREVIEW_OPTIONS.markdown,
		};
	}
	if (typeInfo.category === "csv") {
		return {
			category: "csv",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: BUILTIN_TABLE_PREVIEW_APP_KEY,
			options: BUILTIN_PREVIEW_OPTIONS.csv,
		};
	}
	if (typeInfo.category === "tsv") {
		return {
			category: "tsv",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: BUILTIN_TABLE_PREVIEW_APP_KEY,
			options: BUILTIN_PREVIEW_OPTIONS.tsv,
		};
	}
	if (typeInfo.category === "json") {
		return {
			category: "json",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: "builtin.formatted",
			options: BUILTIN_PREVIEW_OPTIONS.json,
		};
	}
	if (typeInfo.category === "xml") {
		return {
			category: "xml",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: "builtin.formatted",
			options: BUILTIN_PREVIEW_OPTIONS.xml,
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
			defaultMode: "builtin.code",
			options: BUILTIN_PREVIEW_OPTIONS.text,
		};
	}

	return {
		category: typeInfo.category,
		isBlobPreview: false,
		isTextBased: false,
		isEditableText: typeInfo.category === "unknown",
		defaultMode: null,
		options:
			typeInfo.category === "unknown"
				? [
						{
							key: "builtin.try_text",
							mode: "code",
							labelKey: "open_with_try_text",
							icon: PREVIEW_APP_ICON_URLS.file,
						},
					]
				: [],
	};
}

function normalizeConfiguredOption(
	app: ConfiguredPreviewApp,
	category: FilePreviewProfile["category"],
): OpenWithOption | null {
	const provider = getConfiguredPreviewProvider(app);
	if (!provider) {
		return null;
	}

	const mode = getConfiguredPreviewMode(app.key, provider);
	if (!mode) {
		return null;
	}

	return {
		key: app.key,
		mode,
		labelKey: "",
		labels: app.labels ?? undefined,
		icon: getConfiguredPreviewIcon(app, category),
		config: (app.config as Record<string, unknown> | undefined) ?? {},
	};
}

function resolveBuiltinOptionForConfiguredProfile(
	option: OpenWithOption,
	appMap: Map<string, OpenWithOption>,
	availableAppKeys: Set<string>,
) {
	if (option.key.startsWith("builtin.") && !availableAppKeys.has(option.key)) {
		return null;
	}

	return appMap.get(option.key) ?? option;
}

function getConfiguredBuiltinPreviewIcon(
	key: string,
	category: FilePreviewProfile["category"],
) {
	if (key === "builtin.formatted") {
		return category === "xml"
			? PREVIEW_APP_ICON_URLS.xml
			: PREVIEW_APP_ICON_URLS.json;
	}

	return getBuiltinPreviewAppIconUrl(key);
}

function getConfiguredPreviewIcon(
	app: ConfiguredPreviewApp,
	category: FilePreviewProfile["category"],
): string {
	const configuredIcon = app.icon?.trim() ?? "";
	const defaultIcon = getBuiltinPreviewAppIconUrl(app.key);
	const builtinIcon = getConfiguredBuiltinPreviewIcon(app.key, category);

	if (!configuredIcon || configuredIcon === defaultIcon) {
		return builtinIcon;
	}

	return configuredIcon;
}

function getConfiguredPreviewProvider(
	app: ConfiguredPreviewApp,
): PreviewAppProvider | null {
	const provider = app.provider.trim().toLowerCase();
	if (provider === "builtin") {
		return "builtin";
	}
	if (provider === "wopi") {
		return "wopi";
	}
	if (provider === "url_template") {
		return "url_template";
	}
	return null;
}

function isConfiguredPreviewAppEnabled(app: ConfiguredPreviewApp) {
	return app.enabled !== false;
}

function getConfiguredPreviewMode(
	key: string,
	provider: PreviewAppProvider,
): OpenWithOption["mode"] | null {
	if (provider === "url_template") {
		return "url_template";
	}

	if (provider === "wopi") {
		return "wopi";
	}

	switch (key) {
		case "builtin.image":
			return "image";
		case "builtin.video":
			return "video";
		case "builtin.audio":
			return "audio";
		case "builtin.pdf":
			return "pdf";
		case "builtin.markdown":
			return "markdown";
		case BUILTIN_TABLE_PREVIEW_APP_KEY:
			return "table";
		case "builtin.formatted":
			return "formatted";
		case "builtin.code":
		case "builtin.try_text":
			return "code";
		default:
			return null;
	}
}

function matchesConfiguredApp(
	file: PreviewableFileLike,
	app: ConfiguredPreviewApp,
) {
	const extension = getFileExtension(file);
	if (!extension) {
		return false;
	}

	return (app.extensions ?? []).some(
		(candidate) =>
			candidate.trim().replace(/^\./, "").toLowerCase() === extension,
	);
}

function detectConfiguredFilePreviewProfile(
	file: PreviewableFileLike,
	previewApps: PublicPreviewAppsConfig,
): FilePreviewProfile {
	const builtinProfile = detectBuiltinFilePreviewProfile(file);
	const allConfiguredApps = previewApps.apps ?? [];
	const configuredApps = allConfiguredApps.filter(
		isConfiguredPreviewAppEnabled,
	);
	const configuredOptions = configuredApps
		.map((app) => {
			const option = normalizeConfiguredOption(app, builtinProfile.category);
			return option ? ([app.key, option] as const) : null;
		})
		.filter(
			(
				entry,
			): entry is readonly [ConfiguredPreviewApp["key"], OpenWithOption] =>
				entry !== null,
		);
	const appMap = new Map(configuredOptions);
	const availableAppKeys = new Set(configuredApps.map((app) => app.key));
	const matchedConfiguredOptions = configuredApps
		.filter((app) => matchesConfiguredApp(file, app))
		.map((app) => appMap.get(app.key) ?? null)
		.filter((option): option is OpenWithOption => option !== null);

	const builtinOptions = builtinProfile.options
		.map((option) =>
			resolveBuiltinOptionForConfiguredProfile(
				option,
				appMap,
				availableAppKeys,
			),
		)
		.filter((option): option is OpenWithOption => option !== null);
	const options = mergeOpenWithOptions(
		matchedConfiguredOptions,
		builtinOptions,
	);
	const registeredOptions = configuredOptions.map(([, option]) => option);
	const allOptions = mergeOpenWithOptions(options, registeredOptions);
	let defaultMode = matchedConfiguredOptions[0]?.key ?? null;

	if (defaultMode === null && builtinProfile.defaultMode) {
		const builtinDefaultOption = builtinProfile.options.find(
			(option) => option.key === builtinProfile.defaultMode,
		);
		const builtinDefault = builtinDefaultOption
			? resolveBuiltinOptionForConfiguredProfile(
					builtinDefaultOption,
					appMap,
					availableAppKeys,
				)
			: null;
		if (
			builtinDefault &&
			allOptions.some((option) => option.key === builtinDefault.key)
		) {
			defaultMode = builtinDefault.key;
		}
	}

	if (defaultMode === null && options.length > 0) {
		defaultMode = options[0]?.key ?? null;
	}
	if (defaultMode === null && allOptions.length > 0) {
		defaultMode = allOptions[0]?.key ?? null;
	}

	return {
		...builtinProfile,
		defaultMode,
		allOptions,
		options,
	};
}

export function detectFilePreviewProfile(
	file: PreviewableFileLike,
	previewApps?: PublicPreviewAppsConfig | null,
): FilePreviewProfile {
	if (!previewApps) {
		return detectBuiltinFilePreviewProfile(file);
	}
	return detectConfiguredFilePreviewProfile(file, previewApps);
}

export function getAvailableOpenWithOptions(
	file: PreviewableFileLike,
	previewApps?: PublicPreviewAppsConfig | null,
) {
	const profile = detectFilePreviewProfile(file, previewApps);
	return profile.allOptions ?? profile.options;
}

export function getDefaultOpenWith(
	file: PreviewableFileLike,
	previewApps?: PublicPreviewAppsConfig | null,
) {
	return detectFilePreviewProfile(file, previewApps).defaultMode;
}

export function isEditableTextFile(file: PreviewableFileLike) {
	return detectBuiltinFilePreviewProfile(file).isEditableText;
}
