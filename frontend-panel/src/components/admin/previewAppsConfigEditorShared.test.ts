import { describe, expect, it } from "vitest";
import {
	createPreviewAppDraft,
	createPreviewRuleDraft,
	getPreviewAppsConfigIssues,
	getPreviewAppsConfigIssuesFromString,
	parsePreviewAppsConfig,
	removePreviewRuleAppKey,
	renamePreviewRuleAppKey,
	serializePreviewAppsConfig,
} from "@/components/admin/previewAppsConfigEditorShared";
import { PREVIEW_APP_ICON_URLS } from "@/components/common/previewAppIconUrls";

describe("previewAppsConfigEditorShared", () => {
	it("parses and serializes preview app config drafts", () => {
		const draft = parsePreviewAppsConfig(`{
			"version": 1,
			"apps": [
				{
					"key": "builtin.image",
					"icon": "https://cdn.example.com/image.svg",
					"enabled": true,
					"provider": "builtin",
					"labels": {
						"en": "Image preview",
						"zh": "图片预览"
					}
				},
				{
					"key": "custom.viewer",
					"icon": "https://cdn.example.com/jellyfin.svg",
					"enabled": false,
					"provider": "url_template",
					"labels": {
						"en": "Jellyfin"
					},
					"config": {
						"mode": "iframe",
						"url_template": "https://videos.example.com/watch?src={{file_preview_url}}",
						"allowed_origins": ["https://videos.example.com"]
					}
				}
			],
			"rules": [
				{
					"matches": { "categories": ["video"] },
					"apps": ["custom.viewer"],
					"default_app": "custom.viewer"
				}
			]
		}`);

		expect(draft).toMatchObject({
			apps: [
				{
					labels: {
						en: "Image preview",
						zh: "图片预览",
					},
					key: "builtin.image",
					provider: "builtin",
				},
				{
					config: {
						allowed_origins: ["https://videos.example.com"],
						mode: "iframe",
						url_template:
							"https://videos.example.com/watch?src={{file_preview_url}}",
					},
					enabled: false,
					key: "custom.viewer",
					labels: {
						en: "Jellyfin",
					},
					provider: "url_template",
				},
			],
			rules: [
				{
					apps: ["custom.viewer"],
					default_app: "custom.viewer",
					matches: { categories: ["video"] },
				},
			],
			version: 1,
		});

		expect(JSON.parse(serializePreviewAppsConfig(draft))).toMatchObject({
			apps: expect.arrayContaining([
				{
					enabled: true,
					icon: "https://cdn.example.com/image.svg",
					key: "builtin.image",
					labels: {
						en: "Image preview",
						zh: "图片预览",
					},
					provider: "builtin",
				},
				{
					config: {
						allowed_origins: ["https://videos.example.com"],
						mode: "iframe",
						url_template:
							"https://videos.example.com/watch?src={{file_preview_url}}",
					},
					enabled: false,
					icon: "https://cdn.example.com/jellyfin.svg",
					key: "custom.viewer",
					labels: {
						en: "Jellyfin",
					},
					provider: "url_template",
				},
			]),
			version: 1,
		});
	});

	it("treats default icons as empty overrides", () => {
		const draft = parsePreviewAppsConfig(`{
			"version": 1,
			"apps": [
				{
					"key": "builtin.image",
					"icon": "${PREVIEW_APP_ICON_URLS.image}",
					"enabled": true,
					"provider": "builtin",
					"labels": {
						"zh": "图片预览"
					}
				},
				{
					"key": "custom.viewer",
					"icon": "${PREVIEW_APP_ICON_URLS.web}",
					"enabled": true,
					"provider": "url_template",
					"labels": {
						"zh": "外部查看器"
					},
					"config": {
						"mode": "iframe",
						"url_template": "https://viewer.example.com/embed?src={{file_preview_url}}"
					}
				}
			],
			"rules": []
		}`);

		expect(draft.apps).toMatchObject([
			{
				icon: "",
				key: "builtin.image",
			},
			{
				icon: "",
				key: "custom.viewer",
			},
		]);

		expect(JSON.parse(serializePreviewAppsConfig(draft)).apps).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					icon: "",
					key: "builtin.image",
				}),
				expect.objectContaining({
					icon: "",
					key: "custom.viewer",
				}),
			]),
		);
	});

	it("drops non-url icon overrides", () => {
		const draft = parsePreviewAppsConfig(`{
			"version": 1,
			"apps": [
				{
					"key": "custom.viewer",
					"icon": "Globe",
					"enabled": true,
					"provider": "url_template",
					"labels": {
						"zh": "外部查看器"
					},
					"config": {
						"mode": "iframe",
						"url_template": "https://viewer.example.com/embed?src={{file_preview_url}}"
					}
				}
			],
			"rules": []
		}`);

		expect(draft.apps).toMatchObject([
			{
				icon: "",
				key: "custom.viewer",
			},
		]);

		expect(JSON.parse(serializePreviewAppsConfig(draft)).apps).toEqual([
			expect.objectContaining({
				icon: "",
				key: "custom.viewer",
			}),
		]);
	});

	it("does not infer provider from the app key when parsing drafts", () => {
		const draft = parsePreviewAppsConfig(`{
			"version": 1,
			"apps": [
				{
					"key": "custom.viewer",
					"icon": "",
					"enabled": true,
					"labels": {
						"zh": "外部查看器"
					},
					"config": {
						"mode": "iframe",
						"url_template": "https://viewer.example.com/embed?src={{file_preview_url}}"
					}
				}
			],
			"rules": []
		}`);

		expect(draft.apps).toMatchObject([
			{
				key: "custom.viewer",
				provider: "",
			},
		]);
	});

	it("renames and removes rule references when app keys change", () => {
		const rules = [
			{
				apps: ["custom.viewer", "builtin.code"],
				default_app: "custom.viewer",
				matches: {
					categories: ["video"],
					extensions: [],
					mime_prefixes: [],
					mime_types: [],
				},
			},
		];

		expect(
			renamePreviewRuleAppKey(rules, "custom.viewer", "custom.viewer_v2"),
		).toEqual([
			{
				apps: ["custom.viewer_v2", "builtin.code"],
				default_app: "custom.viewer_v2",
				matches: {
					categories: ["video"],
					extensions: [],
					mime_prefixes: [],
					mime_types: [],
				},
			},
		]);

		expect(removePreviewRuleAppKey(rules, "custom.viewer")).toEqual([
			{
				apps: ["builtin.code"],
				default_app: "builtin.code",
				matches: {
					categories: ["video"],
					extensions: [],
					mime_prefixes: [],
					mime_types: [],
				},
			},
		]);
	});

	it("creates useful default app and rule drafts", () => {
		const app = createPreviewAppDraft(["custom.app_1"]);
		expect(app).toMatchObject({
			config: {
				allowed_origins: [],
				mode: "iframe",
				url_template: "",
			},
			icon: "",
			key: "custom.app_2",
			labels: {},
			provider: "url_template",
		});

		expect(
			createPreviewRuleDraft([
				{
					config: {},
					enabled: true,
					icon: "",
					key: "builtin.image",
					label_i18n_key: "open_with_image",
					labels: {},
					provider: "builtin",
				},
			]),
		).toEqual({
			apps: ["builtin.image"],
			default_app: "builtin.image",
			matches: {
				categories: [],
				extensions: [],
				mime_prefixes: [],
				mime_types: [],
			},
		});
	});

	it("reports validation issues for invalid drafts", () => {
		expect(getPreviewAppsConfigIssuesFromString("{bad json")).toEqual([
			{ key: "preview_apps_error_parse" },
		]);

		expect(
			getPreviewAppsConfigIssues({
				apps: [
					{
						config: { mode: "" },
						enabled: true,
						icon: "",
						key: "",
						label_i18n_key: "",
						labels: {},
						provider: "",
					},
					{
						config: {},
						enabled: true,
						icon: "",
						key: "",
						label_i18n_key: "",
						labels: {},
						provider: "url_template",
					},
					{
						config: {
							mode: "",
						},
						enabled: true,
						icon: "",
						key: "custom.onlyoffice",
						label_i18n_key: "",
						labels: {
							zh: "OnlyOffice",
						},
						provider: "wopi",
					},
				],
				rules: [
					{
						apps: ["missing.app"],
						default_app: "builtin.image",
						matches: {
							categories: [],
							extensions: [],
							mime_prefixes: [],
							mime_types: [],
						},
					},
				],
				version: 99,
			}).map((issue) => issue.key),
		).toEqual(
			expect.arrayContaining([
				"preview_apps_error_version_mismatch",
				"preview_apps_error_app_key_required",
				"preview_apps_error_app_label_required",
				"preview_apps_error_app_provider_required",
				"preview_apps_error_url_template_mode_required",
				"preview_apps_error_url_template_required",
				"preview_apps_error_builtin_required",
				"preview_apps_error_rule_unknown_app",
				"preview_apps_error_rule_default_missing",
				"preview_apps_error_wopi_mode_required",
				"preview_apps_error_wopi_target_required",
			]),
		);
	});
});
