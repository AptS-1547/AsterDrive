import { PREVIEW_APP_ICON_URLS } from "@/components/common/previewAppIconUrls";
import {
	BUILTIN_TABLE_PREVIEW_APP_KEY,
	isTablePreviewAppKey,
	normalizeTablePreviewDelimiter,
} from "@/lib/tablePreview";
import type { PreviewAppProvider } from "@/types/api";

export { isTablePreviewAppKey } from "@/lib/tablePreview";

export const PREVIEW_APPS_CONFIG_KEY = "frontend_preview_apps_json";
export const PREVIEW_APPS_CONFIG_VERSION = 1;
export const PREVIEW_APP_PROTECTED_BUILTIN_KEYS = [
	"builtin.image",
	"builtin.video",
	"builtin.audio",
	"builtin.pdf",
	"builtin.markdown",
	BUILTIN_TABLE_PREVIEW_APP_KEY,
	"builtin.formatted_json",
	"builtin.formatted_xml",
	"builtin.code",
	"builtin.try_text",
] as const;

export interface PreviewAppsEditorMatch {
	categories: string[];
	extensions: string[];
	mime_prefixes: string[];
	mime_types: string[];
}

export interface PreviewAppsEditorRule {
	apps: string[];
	default_app: string;
	matches: PreviewAppsEditorMatch;
}

export interface PreviewAppsEditorApp {
	config: Record<string, unknown>;
	enabled: boolean;
	icon: string;
	key: string;
	label_i18n_key: string;
	labels: Record<string, string>;
	provider: PreviewAppProviderValue;
}

export interface PreviewAppsEditorConfig {
	apps: PreviewAppsEditorApp[];
	rules: PreviewAppsEditorRule[];
	version: number;
}

export interface PreviewAppsValidationIssue {
	key: string;
	values?: Record<string, number | string>;
}

export type PreviewAppProviderValue = PreviewAppProvider | "";

const PREVIEW_APP_KEY_META: Record<string, { icon: string; labelKey: string }> =
	{
		"builtin.audio": {
			icon: PREVIEW_APP_ICON_URLS.audio,
			labelKey: "preview_apps_provider_audio",
		},
		"builtin.code": {
			icon: PREVIEW_APP_ICON_URLS.code,
			labelKey: "preview_apps_provider_code",
		},
		"builtin.formatted_json": {
			icon: PREVIEW_APP_ICON_URLS.json,
			labelKey: "preview_apps_provider_formatted_json",
		},
		"builtin.formatted_xml": {
			icon: PREVIEW_APP_ICON_URLS.xml,
			labelKey: "preview_apps_provider_formatted_xml",
		},
		"builtin.image": {
			icon: PREVIEW_APP_ICON_URLS.image,
			labelKey: "preview_apps_provider_image",
		},
		"builtin.markdown": {
			icon: PREVIEW_APP_ICON_URLS.markdown,
			labelKey: "preview_apps_provider_markdown",
		},
		"builtin.office_google": {
			icon: PREVIEW_APP_ICON_URLS.googleDrive,
			labelKey: "preview_apps_provider_url_template",
		},
		"builtin.office_microsoft": {
			icon: PREVIEW_APP_ICON_URLS.microsoftOnedrive,
			labelKey: "preview_apps_provider_url_template",
		},
		"builtin.pdf": {
			icon: PREVIEW_APP_ICON_URLS.pdf,
			labelKey: "preview_apps_provider_pdf",
		},
		[BUILTIN_TABLE_PREVIEW_APP_KEY]: {
			icon: PREVIEW_APP_ICON_URLS.table,
			labelKey: "preview_apps_provider_table",
		},
		"builtin.try_text": {
			icon: PREVIEW_APP_ICON_URLS.file,
			labelKey: "preview_apps_provider_code",
		},
		"builtin.video": {
			icon: PREVIEW_APP_ICON_URLS.video,
			labelKey: "preview_apps_provider_video",
		},
	};
const PREVIEW_APP_LEGACY_LABELS: Record<string, Record<string, string>> = {
	"builtin.audio": { en: "Audio preview", zh: "音频预览" },
	"builtin.code": { en: "Source view", zh: "源码视图" },
	"builtin.formatted_json": { en: "Formatted view", zh: "格式化视图" },
	"builtin.formatted_xml": { en: "Formatted view", zh: "格式化视图" },
	"builtin.image": { en: "Image preview", zh: "图片预览" },
	"builtin.markdown": { en: "Markdown preview", zh: "Markdown 预览" },
	"builtin.office_google": {
		en: "Google Viewer",
		zh: "Google 预览器",
	},
	"builtin.office_microsoft": {
		en: "Microsoft Viewer",
		zh: "Microsoft 预览器",
	},
	"builtin.pdf": { en: "PDF preview", zh: "PDF 预览" },
	[BUILTIN_TABLE_PREVIEW_APP_KEY]: { en: "Table preview", zh: "表格预览" },
	"builtin.try_text": { en: "Open as text", zh: "以文本方式打开" },
	"builtin.video": { en: "Video preview", zh: "视频预览" },
	open_with_audio: { en: "Audio preview", zh: "音频预览" },
	open_with_code: { en: "Source view", zh: "源码视图" },
	open_with_formatted: { en: "Formatted view", zh: "格式化视图" },
	open_with_image: { en: "Image preview", zh: "图片预览" },
	open_with_markdown: { en: "Markdown preview", zh: "Markdown 预览" },
	open_with_office_google: {
		en: "Google Viewer",
		zh: "Google 预览器",
	},
	open_with_office_microsoft: {
		en: "Microsoft Viewer",
		zh: "Microsoft 预览器",
	},
	open_with_pdf: { en: "PDF preview", zh: "PDF 预览" },
	open_with_table: { en: "Table preview", zh: "表格预览" },
	open_with_try_text: { en: "Open as text", zh: "以文本方式打开" },
	open_with_video: { en: "Video preview", zh: "视频预览" },
};

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isProtectedBuiltinPreviewApp(key: string) {
	return PREVIEW_APP_PROTECTED_BUILTIN_KEYS.includes(
		key.trim() as (typeof PREVIEW_APP_PROTECTED_BUILTIN_KEYS)[number],
	);
}

function readString(value: unknown) {
	return typeof value === "string" ? value : "";
}

function readPreviewAppProvider(value: unknown): PreviewAppProviderValue {
	const normalized = readString(value).trim().toLowerCase();
	if (normalized === "builtin") {
		return "builtin";
	}
	if (normalized === "url_template") {
		return "url_template";
	}
	if (normalized === "wopi") {
		return "wopi";
	}
	return "";
}

function readBoolean(value: unknown, fallback = false) {
	return typeof value === "boolean" ? value : fallback;
}

function readStringList(value: unknown) {
	if (!Array.isArray(value)) {
		return [];
	}

	return value
		.map((item) => readString(item).trim())
		.filter(
			(item, index, items) => item.length > 0 && items.indexOf(item) === index,
		);
}

function readStringMap(value: unknown) {
	if (!isRecord(value)) {
		return {};
	}

	const next: Record<string, string> = {};
	for (const [key, item] of Object.entries(value)) {
		const normalizedKey = key.trim().toLowerCase().replaceAll("_", "-");
		const normalizedValue = readString(item).trim();
		if (!normalizedKey || !normalizedValue) {
			continue;
		}
		next[normalizedKey] = normalizedValue;
	}

	return next;
}

const ICON_URL_PATTERN =
	/^(https?:\/\/|\/\/|\/(?!\/)|\.\/|\.\.\/|data:image\/|blob:)/i;

function isPreviewAppIconUrl(value: string) {
	return ICON_URL_PATTERN.test(value.trim());
}

function cloneConfigMap(value: unknown) {
	if (!isRecord(value)) {
		return {};
	}

	return { ...value };
}

export function getPreviewAppProvider(
	provider?: unknown,
): PreviewAppProviderValue {
	return readPreviewAppProvider(provider);
}

export function getPreviewAppDefaultIcon(key: string, provider?: unknown) {
	if (getPreviewAppProvider(provider) === "wopi") {
		return PREVIEW_APP_ICON_URLS.web;
	}

	return PREVIEW_APP_KEY_META[key.trim()]?.icon ?? PREVIEW_APP_ICON_URLS.web;
}

function normalizePreviewAppIconOverride(
	key: string,
	value: unknown,
	provider?: unknown,
) {
	const icon = readString(value).trim();
	if (!icon) {
		return "";
	}

	if (!isPreviewAppIconUrl(icon)) {
		return "";
	}

	return icon === getPreviewAppDefaultIcon(key, provider) ? "" : icon;
}

export function getPreviewAppKindLabelKey(key: string, provider?: unknown) {
	const resolvedProvider = getPreviewAppProvider(provider);
	if (resolvedProvider === "wopi") {
		return "preview_apps_provider_wopi";
	}

	return (
		PREVIEW_APP_KEY_META[key.trim()]?.labelKey ??
		"preview_apps_provider_url_template"
	);
}

function getLegacyPreviewAppLabels(
	key: string,
	labelI18nKey: string,
): Record<string, string> {
	const matched =
		PREVIEW_APP_LEGACY_LABELS[key.trim()] ??
		PREVIEW_APP_LEGACY_LABELS[labelI18nKey.trim()];
	return matched ? { ...matched } : {};
}

function normalizeApp(value: unknown): PreviewAppsEditorApp {
	if (!isRecord(value)) {
		return {
			config: {},
			enabled: true,
			icon: "",
			key: "",
			label_i18n_key: "",
			labels: {},
			provider: "",
		};
	}

	const key = readString(value.key);
	const provider = getPreviewAppProvider(value.provider);
	const labelI18nKey = readString(value.label_i18n_key);
	const labels = readStringMap(value.labels);
	const config = cloneConfigMap(value.config);

	if (isTablePreviewAppKey(key)) {
		config.delimiter = normalizeTablePreviewDelimiter(config.delimiter);
	}

	return {
		config,
		enabled: readBoolean(value.enabled, true),
		icon: normalizePreviewAppIconOverride(key, value.icon, provider),
		key,
		label_i18n_key: labelI18nKey,
		labels:
			Object.keys(labels).length > 0
				? labels
				: getLegacyPreviewAppLabels(key, labelI18nKey),
		provider,
	};
}

function normalizeMatch(value: unknown): PreviewAppsEditorMatch {
	if (!isRecord(value)) {
		return {
			categories: [],
			extensions: [],
			mime_prefixes: [],
			mime_types: [],
		};
	}

	return {
		categories: readStringList(value.categories),
		extensions: readStringList(value.extensions),
		mime_prefixes: readStringList(value.mime_prefixes),
		mime_types: readStringList(value.mime_types),
	};
}

function normalizeRule(value: unknown): PreviewAppsEditorRule {
	if (!isRecord(value)) {
		return {
			apps: [],
			default_app: "",
			matches: normalizeMatch(null),
		};
	}

	return {
		apps: readStringList(value.apps),
		default_app: readString(value.default_app),
		matches: normalizeMatch(value.matches),
	};
}

export function parsePreviewAppsDelimitedInput(value: string) {
	return value
		.split(",")
		.map((item) => item.trim())
		.filter(
			(item, index, items) => item.length > 0 && items.indexOf(item) === index,
		);
}

export function formatPreviewAppsDelimitedInput(values: string[]) {
	return values.join(", ");
}

export function isProtectedBuiltinPreviewAppKey(key: string) {
	return isProtectedBuiltinPreviewApp(key);
}

export function isExternalPreviewAppKey(key: string) {
	return !isProtectedBuiltinPreviewApp(key);
}

export function isUrlTemplatePreviewApp(app: PreviewAppsEditorApp) {
	return app.provider === "url_template";
}

export function isWopiPreviewApp(app: PreviewAppsEditorApp) {
	return app.provider === "wopi";
}

export function parsePreviewAppsConfig(value: string): PreviewAppsEditorConfig {
	const parsed = JSON.parse(value) as unknown;
	if (!isRecord(parsed)) {
		throw new Error("preview apps config must be an object");
	}

	return {
		apps: Array.isArray(parsed.apps) ? parsed.apps.map(normalizeApp) : [],
		rules: Array.isArray(parsed.rules) ? parsed.rules.map(normalizeRule) : [],
		version:
			typeof parsed.version === "number"
				? parsed.version
				: PREVIEW_APPS_CONFIG_VERSION,
	};
}

function pruneConfigValue(value: unknown): unknown {
	if (typeof value === "string") {
		const trimmed = value.trim();
		return trimmed.length > 0 ? trimmed : undefined;
	}

	if (Array.isArray(value)) {
		const items = value
			.map((item) => pruneConfigValue(item))
			.filter((item) => item !== undefined);
		return items.length > 0 ? items : undefined;
	}

	if (value === null || value === undefined) {
		return undefined;
	}

	return value;
}

function pruneConfigMap(config: Record<string, unknown>) {
	const next: Record<string, unknown> = {};

	for (const [key, value] of Object.entries(config)) {
		const normalized = pruneConfigValue(value);
		if (normalized !== undefined) {
			next[key] = normalized;
		}
	}

	return next;
}

function pruneStringMap(values: Record<string, string>) {
	const next: Record<string, string> = {};

	for (const [key, value] of Object.entries(values)) {
		const normalizedKey = key.trim().toLowerCase().replaceAll("_", "-");
		const normalizedValue = value.trim();
		if (!normalizedKey || !normalizedValue) {
			continue;
		}
		next[normalizedKey] = normalizedValue;
	}

	return next;
}

export function serializePreviewAppsConfig(config: PreviewAppsEditorConfig) {
	return JSON.stringify(
		{
			version: config.version,
			apps: config.apps.map((app) => {
				const key = app.key.trim();
				const nextConfig = pruneConfigMap(app.config);
				const nextIcon = normalizePreviewAppIconOverride(
					key,
					app.icon,
					app.provider,
				);
				const nextLabels = pruneStringMap(app.labels);
				return {
					...(Object.keys(nextConfig).length > 0 ? { config: nextConfig } : {}),
					enabled: app.enabled,
					icon: nextIcon,
					key,
					provider: app.provider,
					...(Object.keys(nextLabels).length > 0 ? { labels: nextLabels } : {}),
					...(app.label_i18n_key.trim()
						? { label_i18n_key: app.label_i18n_key.trim() }
						: {}),
				};
			}),
			rules: config.rules.map((rule) => ({
				apps: rule.apps.map((appKey) => appKey.trim()),
				...(rule.default_app.trim()
					? { default_app: rule.default_app.trim() }
					: {}),
				matches: {
					categories: [...rule.matches.categories],
					extensions: [...rule.matches.extensions],
					mime_prefixes: [...rule.matches.mime_prefixes],
					mime_types: [...rule.matches.mime_types],
				},
			})),
		},
		null,
		2,
	);
}

export function getPreviewAppsConfigIssues(
	config: PreviewAppsEditorConfig,
): PreviewAppsValidationIssue[] {
	const issues: PreviewAppsValidationIssue[] = [];

	if (config.version !== PREVIEW_APPS_CONFIG_VERSION) {
		issues.push({
			key: "preview_apps_error_version_mismatch",
			values: { version: PREVIEW_APPS_CONFIG_VERSION },
		});
	}

	if (config.apps.length === 0) {
		issues.push({ key: "preview_apps_error_no_apps" });
	}

	const keyCounts = new Map<string, number>();
	for (const app of config.apps) {
		const key = app.key.trim();
		if (!key) {
			continue;
		}
		keyCounts.set(key, (keyCounts.get(key) ?? 0) + 1);
	}

	const appKeys = new Set(
		config.apps.map((app) => app.key.trim()).filter((key) => key.length > 0),
	);

	for (const [index, app] of config.apps.entries()) {
		const appNumber = index + 1;
		const key = app.key.trim();
		if (!key) {
			issues.push({
				key: "preview_apps_error_app_key_required",
				values: { index: appNumber },
			});
		} else if ((keyCounts.get(key) ?? 0) > 1) {
			issues.push({
				key: "preview_apps_error_app_key_duplicate",
				values: { index: appNumber, key },
			});
		}

		if (
			!app.label_i18n_key.trim() &&
			Object.values(app.labels).every((value) => value.trim().length === 0)
		) {
			issues.push({
				key: "preview_apps_error_app_label_required",
				values: { index: appNumber },
			});
		}

		const provider = getPreviewAppProvider(app.provider);
		if (!provider) {
			issues.push({
				key: "preview_apps_error_app_provider_required",
				values: { index: appNumber },
			});
		}
		if (provider === "url_template") {
			const mode =
				typeof app.config.mode === "string" ? app.config.mode.trim() : "";
			if (!mode) {
				issues.push({
					key: "preview_apps_error_url_template_mode_required",
					values: { index: appNumber },
				});
			}

			const urlTemplate =
				typeof app.config.url_template === "string"
					? app.config.url_template.trim()
					: "";
			if (!urlTemplate) {
				issues.push({
					key: "preview_apps_error_url_template_required",
					values: { index: appNumber },
				});
			}
		}

		if (provider === "wopi") {
			const mode =
				typeof app.config.mode === "string" ? app.config.mode.trim() : "";
			if (!mode) {
				issues.push({
					key: "preview_apps_error_wopi_mode_required",
					values: { index: appNumber },
				});
			}

			const actionUrl =
				typeof app.config.action_url === "string"
					? app.config.action_url.trim()
					: "";
			const actionUrlTemplate =
				typeof app.config.action_url_template === "string"
					? app.config.action_url_template.trim()
					: "";
			const discoveryUrl =
				typeof app.config.discovery_url === "string"
					? app.config.discovery_url.trim()
					: "";
			if (!actionUrl && !actionUrlTemplate && !discoveryUrl) {
				issues.push({
					key: "preview_apps_error_wopi_target_required",
					values: { index: appNumber },
				});
			}
		}
	}

	for (const builtinKey of PREVIEW_APP_PROTECTED_BUILTIN_KEYS) {
		if (!appKeys.has(builtinKey)) {
			issues.push({
				key: "preview_apps_error_builtin_required",
				values: { key: builtinKey },
			});
		}
	}

	for (const [index, rule] of config.rules.entries()) {
		const ruleNumber = index + 1;
		if (rule.apps.length === 0) {
			issues.push({
				key: "preview_apps_error_rule_apps_required",
				values: { index: ruleNumber },
			});
		}

		for (const appKey of rule.apps) {
			if (!appKeys.has(appKey)) {
				issues.push({
					key: "preview_apps_error_rule_unknown_app",
					values: { appKey, index: ruleNumber },
				});
			}
		}

		if (
			rule.default_app.trim() &&
			!rule.apps.includes(rule.default_app.trim())
		) {
			issues.push({
				key: "preview_apps_error_rule_default_missing",
				values: {
					appKey: rule.default_app.trim(),
					index: ruleNumber,
				},
			});
		}
	}

	return issues;
}

export function getPreviewAppsConfigIssuesFromString(value: string) {
	try {
		return getPreviewAppsConfigIssues(parsePreviewAppsConfig(value));
	} catch {
		return [{ key: "preview_apps_error_parse" }];
	}
}

function getNextCustomKey(existingKeys: string[]) {
	let index = 1;
	let candidate = `custom.app_${index}`;

	while (existingKeys.includes(candidate)) {
		index += 1;
		candidate = `custom.app_${index}`;
	}

	return candidate;
}

export function createPreviewAppDraft(
	existingKeys: string[],
): PreviewAppsEditorApp {
	return {
		config: {
			allowed_origins: [],
			mode: "iframe",
			url_template: "",
		},
		enabled: true,
		icon: "",
		key: getNextCustomKey(existingKeys),
		label_i18n_key: "",
		labels: {},
		provider: "url_template",
	};
}

export function createPreviewRuleDraft(
	apps: PreviewAppsEditorApp[],
): PreviewAppsEditorRule {
	const firstAppKey = apps.find((app) => app.key.trim())?.key.trim() ?? "";

	return {
		apps: firstAppKey ? [firstAppKey] : [],
		default_app: firstAppKey,
		matches: {
			categories: [],
			extensions: [],
			mime_prefixes: [],
			mime_types: [],
		},
	};
}

export function movePreviewEditorItem<T>(
	items: T[],
	index: number,
	direction: -1 | 1,
) {
	const targetIndex = index + direction;
	if (targetIndex < 0 || targetIndex >= items.length) {
		return items;
	}

	const nextItems = [...items];
	const [item] = nextItems.splice(index, 1);
	if (item === undefined) {
		return items;
	}
	nextItems.splice(targetIndex, 0, item);
	return nextItems;
}

export function renamePreviewRuleAppKey(
	rules: PreviewAppsEditorRule[],
	previousKey: string,
	nextKey: string,
) {
	return rules.map((rule) => {
		const apps = rule.apps.map((appKey) =>
			appKey === previousKey ? nextKey : appKey,
		);
		const uniqueApps = apps.filter(
			(appKey, index, items) =>
				appKey.length > 0 && items.indexOf(appKey) === index,
		);
		const defaultApp =
			rule.default_app === previousKey ? nextKey : rule.default_app;

		return {
			...rule,
			apps: uniqueApps,
			default_app: uniqueApps.includes(defaultApp)
				? defaultApp
				: (uniqueApps[0] ?? ""),
		};
	});
}

export function removePreviewRuleAppKey(
	rules: PreviewAppsEditorRule[],
	key: string,
) {
	return rules.map((rule) => {
		const apps = rule.apps.filter((appKey) => appKey !== key);
		return {
			...rule,
			apps,
			default_app: apps.includes(rule.default_app)
				? rule.default_app
				: (apps[0] ?? ""),
		};
	});
}
