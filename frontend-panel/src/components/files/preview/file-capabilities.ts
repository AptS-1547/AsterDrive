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
	PublicPreviewAppRule,
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

export const OPEN_WITH_OPTIONS: Record<string, OpenWithOption[]> = {
	image: [
		{
			key: "image",
			mode: "image",
			labelKey: "open_with_image",
			icon: PREVIEW_APP_ICON_URLS.image,
		},
	],
	video: [
		{
			key: "video",
			mode: "video",
			labelKey: "open_with_video",
			icon: PREVIEW_APP_ICON_URLS.video,
		},
	],
	audio: [
		{
			key: "audio",
			mode: "audio",
			labelKey: "open_with_audio",
			icon: PREVIEW_APP_ICON_URLS.audio,
		},
	],
	pdf: [
		{
			key: "pdf",
			mode: "pdf",
			labelKey: "open_with_pdf",
			icon: PREVIEW_APP_ICON_URLS.pdf,
		},
	],
	document: [
		{
			key: "office_microsoft",
			mode: "url_template",
			labelKey: "open_with_office_microsoft",
			icon: PREVIEW_APP_ICON_URLS.microsoftOnedrive,
			config: MICROSOFT_VIEWER_CONFIG,
		},
		{
			key: "office_google",
			mode: "url_template",
			labelKey: "open_with_office_google",
			icon: PREVIEW_APP_ICON_URLS.googleDrive,
			config: GOOGLE_VIEWER_CONFIG,
		},
	],
	spreadsheet: [
		{
			key: "office_microsoft",
			mode: "url_template",
			labelKey: "open_with_office_microsoft",
			icon: PREVIEW_APP_ICON_URLS.microsoftOnedrive,
			config: MICROSOFT_VIEWER_CONFIG,
		},
		{
			key: "office_google",
			mode: "url_template",
			labelKey: "open_with_office_google",
			icon: PREVIEW_APP_ICON_URLS.googleDrive,
			config: GOOGLE_VIEWER_CONFIG,
		},
	],
	presentation: [
		{
			key: "office_microsoft",
			mode: "url_template",
			labelKey: "open_with_office_microsoft",
			icon: PREVIEW_APP_ICON_URLS.microsoftOnedrive,
			config: MICROSOFT_VIEWER_CONFIG,
		},
		{
			key: "office_google",
			mode: "url_template",
			labelKey: "open_with_office_google",
			icon: PREVIEW_APP_ICON_URLS.googleDrive,
			config: GOOGLE_VIEWER_CONFIG,
		},
	],
	markdown: [
		{
			key: "markdown",
			mode: "markdown",
			labelKey: "open_with_markdown",
			icon: PREVIEW_APP_ICON_URLS.markdown,
		},
		{
			key: "code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
	csv: [
		{
			key: "table",
			mode: "table",
			labelKey: "open_with_table",
			icon: PREVIEW_APP_ICON_URLS.table,
			config: { delimiter: DEFAULT_TABLE_PREVIEW_DELIMITER },
		},
		{
			key: "code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
	tsv: [
		{
			key: "table",
			mode: "table",
			labelKey: "open_with_table",
			icon: PREVIEW_APP_ICON_URLS.table,
			config: { delimiter: DEFAULT_TABLE_PREVIEW_DELIMITER },
		},
		{
			key: "code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
	json: [
		{
			key: "formatted_json",
			mode: "formatted_json",
			labelKey: "open_with_formatted",
			icon: PREVIEW_APP_ICON_URLS.json,
		},
		{
			key: "code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
	xml: [
		{
			key: "formatted_xml",
			mode: "formatted_xml",
			labelKey: "open_with_formatted",
			icon: PREVIEW_APP_ICON_URLS.xml,
		},
		{
			key: "code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
	text: [
		{
			key: "code",
			mode: "code",
			labelKey: "open_with_code",
			icon: PREVIEW_APP_ICON_URLS.code,
		},
	],
};

const BUILTIN_KEY_BY_LEGACY_OPTION_KEY: Partial<Record<string, string>> = {
	audio: "builtin.audio",
	code: "builtin.code",
	formatted_json: "builtin.formatted_json",
	formatted_xml: "builtin.formatted_xml",
	image: "builtin.image",
	markdown: "builtin.markdown",
	office_google: "builtin.office_google",
	office_microsoft: "builtin.office_microsoft",
	pdf: "builtin.pdf",
	table: BUILTIN_TABLE_PREVIEW_APP_KEY,
	try_text: "builtin.try_text",
	video: "builtin.video",
};

type ConfiguredPreviewApp = PublicPreviewAppDefinition;
type ConfiguredPreviewRule = PublicPreviewAppRule;

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
	if (ext === "svg") {
		return { category: "image", icon: "FileText", color: "text-sky-500" };
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

function detectLegacyFilePreviewProfile(
	file: PreviewableFileLike,
): FilePreviewProfile {
	const typeInfo = getFileTypeInfo(file);
	const { ext } = getExtension(file.name);
	const isOpenDocument = ext === "odt" || ext === "ods" || ext === "odp";

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
	if (typeInfo.category === "document") {
		return {
			category: "document",
			isBlobPreview: false,
			isTextBased: false,
			isEditableText: false,
			defaultMode: isOpenDocument ? "office_google" : "office_microsoft",
			options: isOpenDocument
				? [OPEN_WITH_OPTIONS.document[1]].filter(
						(option): option is (typeof OPEN_WITH_OPTIONS.document)[number] =>
							option !== undefined,
					)
				: OPEN_WITH_OPTIONS.document,
		};
	}
	if (typeInfo.category === "spreadsheet") {
		return {
			category: "spreadsheet",
			isBlobPreview: false,
			isTextBased: false,
			isEditableText: false,
			defaultMode: isOpenDocument ? "office_google" : "office_microsoft",
			options: isOpenDocument
				? [OPEN_WITH_OPTIONS.spreadsheet[1]].filter(
						(
							option,
						): option is (typeof OPEN_WITH_OPTIONS.spreadsheet)[number] =>
							option !== undefined,
					)
				: OPEN_WITH_OPTIONS.spreadsheet,
		};
	}
	if (typeInfo.category === "presentation") {
		return {
			category: "presentation",
			isBlobPreview: false,
			isTextBased: false,
			isEditableText: false,
			defaultMode: isOpenDocument ? "office_google" : "office_microsoft",
			options: isOpenDocument
				? [OPEN_WITH_OPTIONS.presentation[1]].filter(
						(
							option,
						): option is (typeof OPEN_WITH_OPTIONS.presentation)[number] =>
							option !== undefined,
					)
				: OPEN_WITH_OPTIONS.presentation,
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
			defaultMode: "formatted_json",
			options: OPEN_WITH_OPTIONS.json,
		};
	}
	if (typeInfo.category === "xml") {
		return {
			category: "xml",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: "formatted_xml",
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
		isEditableText: typeInfo.category === "unknown",
		defaultMode: null,
		options:
			typeInfo.category === "unknown"
				? [
						{
							key: "try_text",
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
		labelKey: app.label_i18n_key ?? "",
		labels: app.labels ?? undefined,
		icon: app.icon?.trim() || getConfiguredPreviewIcon(app.key),
		config: (app.config as Record<string, unknown> | undefined) ?? {},
	};
}

function normalizeLegacyOptionForConfiguredProfile(
	option: OpenWithOption,
	appMap: Map<string, OpenWithOption>,
	disabledKeys = new Set<string>(),
) {
	const builtinKey = BUILTIN_KEY_BY_LEGACY_OPTION_KEY[option.key];
	if (!builtinKey) {
		return option;
	}

	if (disabledKeys.has(builtinKey)) {
		return null;
	}

	return appMap.get(builtinKey) ?? option;
}

function getConfiguredPreviewIcon(key: string): string {
	return getBuiltinPreviewAppIconUrl(key);
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
		case "builtin.formatted_json":
			return "formatted_json";
		case "builtin.formatted_xml":
			return "formatted_xml";
		case "builtin.code":
		case "builtin.try_text":
			return "code";
		default:
			return null;
	}
}

function matchesConfiguredRule(
	file: PreviewableFileLike,
	category: FilePreviewProfile["category"],
	rule: ConfiguredPreviewRule,
) {
	const extension = getFileExtension(file);
	const mimeType = file.mime_type.toLowerCase();
	const matches = rule.matches ?? {};
	const extensions: string[] = matches.extensions ?? [];
	const mimeTypes: string[] = matches.mime_types ?? [];
	const mimePrefixes: string[] = matches.mime_prefixes ?? [];
	const categories: string[] = matches.categories ?? [];
	const hasConditions =
		extensions.length > 0 ||
		mimeTypes.length > 0 ||
		mimePrefixes.length > 0 ||
		categories.length > 0;

	if (!hasConditions) return true;

	const matchesExtension =
		extensions.length > 0 &&
		extensions.some((candidate) => candidate === extension);
	const matchesMimeType =
		mimeTypes.length > 0 &&
		mimeTypes.some((candidate) => candidate === mimeType);
	const matchesMimePrefix =
		mimePrefixes.length > 0 &&
		mimePrefixes.some((candidate) => mimeType.startsWith(candidate));
	const matchesCategory =
		categories.length > 0 &&
		categories.some((candidate) => candidate === category);

	return (
		matchesExtension || matchesMimeType || matchesMimePrefix || matchesCategory
	);
}

function detectConfiguredFilePreviewProfile(
	file: PreviewableFileLike,
	previewApps: PublicPreviewAppsConfig,
): FilePreviewProfile {
	const legacyProfile = detectLegacyFilePreviewProfile(file);
	const allConfiguredApps = previewApps.apps ?? [];
	const disabledAppKeys = new Set(
		allConfiguredApps
			.filter((app) => !isConfiguredPreviewAppEnabled(app))
			.map((app) => app.key),
	);
	const configuredApps = allConfiguredApps.filter(
		isConfiguredPreviewAppEnabled,
	);
	const configuredRules = previewApps.rules ?? [];
	const configuredOptions = configuredApps
		.map((app) => {
			const option = normalizeConfiguredOption(app);
			return option ? ([app.key, option] as const) : null;
		})
		.filter(
			(
				entry,
			): entry is readonly [ConfiguredPreviewApp["key"], OpenWithOption] =>
				entry !== null,
		);
	const appMap = new Map(configuredOptions);
	const options: OpenWithOption[] = [];
	let defaultMode: string | null = null;

	for (const rule of configuredRules) {
		if (!matchesConfiguredRule(file, legacyProfile.category, rule)) {
			continue;
		}

		for (const appKey of rule.apps ?? []) {
			const option = appMap.get(appKey);
			if (
				!option ||
				options.some((candidate) => candidate.key === option.key)
			) {
				continue;
			}
			options.push(option);
		}
	}

	const fallbackOptions = legacyProfile.options
		.map((option) =>
			normalizeLegacyOptionForConfiguredProfile(
				option,
				appMap,
				disabledAppKeys,
			),
		)
		.filter((option): option is OpenWithOption => option !== null);
	const registeredOptions = configuredOptions.map(([, option]) => option);
	const allOptions = mergeOpenWithOptions(
		options,
		registeredOptions,
		fallbackOptions,
	);

	for (const rule of configuredRules) {
		if (!matchesConfiguredRule(file, legacyProfile.category, rule)) {
			continue;
		}
		if (
			defaultMode === null &&
			rule.default_app &&
			allOptions.some((option) => option.key === rule.default_app)
		) {
			defaultMode = rule.default_app;
		}
	}

	if (defaultMode === null && legacyProfile.defaultMode) {
		const legacyDefaultOption = legacyProfile.options.find(
			(option) => option.key === legacyProfile.defaultMode,
		);
		const legacyDefault = legacyDefaultOption
			? normalizeLegacyOptionForConfiguredProfile(legacyDefaultOption, appMap)
			: null;
		if (
			legacyDefault &&
			allOptions.some((option) => option.key === legacyDefault.key)
		) {
			defaultMode = legacyDefault.key;
		}
	}

	if (defaultMode === null && options.length > 0) {
		defaultMode = options[0]?.key ?? null;
	}
	if (defaultMode === null && allOptions.length > 0) {
		defaultMode = allOptions[0]?.key ?? null;
	}

	return {
		...legacyProfile,
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
		return detectLegacyFilePreviewProfile(file);
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
	return detectLegacyFilePreviewProfile(file).isEditableText;
}
