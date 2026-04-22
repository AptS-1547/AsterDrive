import { describe, expect, it } from "vitest";
import {
	getMediaProcessingConfigIssues,
	getMediaProcessingConfigIssuesFromString,
	MEDIA_PROCESSING_DEFAULT_VIPS_EXTENSIONS,
	parseMediaProcessingConfig,
	serializeMediaProcessingConfig,
} from "@/components/admin/mediaProcessingConfigEditorShared";

describe("mediaProcessingConfigEditorShared", () => {
	it("parses and serializes fixed-order processor configs", () => {
		const draft = parseMediaProcessingConfig(`{
			"version": 1,
			"processors": [
				{
					"kind": "vips_cli",
					"enabled": true,
					"extensions": ["heic", ".heif"]
				},
				{
					"kind": "images",
					"enabled": true
				}
			]
		}`);

		expect(draft).toEqual({
			processors: [
				{
					config: {
						command: "vips",
					},
					enabled: true,
					extensions: ["heic", "heif"],
					kind: "vips_cli",
				},
				{
					config: {
						command: "",
					},
					enabled: true,
					extensions: [],
					kind: "images",
				},
			],
			version: 1,
		});

		expect(JSON.parse(serializeMediaProcessingConfig(draft))).toEqual({
			processors: [
				{
					config: {
						command: "vips",
					},
					enabled: true,
					extensions: ["heic", "heif"],
					kind: "vips_cli",
				},
				{
					enabled: true,
					kind: "images",
				},
			],
			version: 1,
		});
	});

	it("fills default vips extensions when the processor is missing from config", () => {
		const draft = parseMediaProcessingConfig(`{
			"version": 1,
			"processors": []
		}`);

		expect(draft.processors[0]).toEqual({
			config: {
				command: "vips",
			},
			enabled: false,
			extensions: [...MEDIA_PROCESSING_DEFAULT_VIPS_EXTENSIONS],
			kind: "vips_cli",
		});
	});

	it("reports validation issues for invalid drafts", () => {
		expect(getMediaProcessingConfigIssuesFromString("{bad json")).toEqual([
			{ key: "media_processing_error_parse" },
		]);

		expect(
			getMediaProcessingConfigIssues({
				processors: [
					{
						config: {
							command: "",
						},
						enabled: false,
						extensions: [],
						kind: "vips_cli",
					},
					{
						config: {
							command: "",
						},
						enabled: false,
						extensions: [],
						kind: "images",
					},
				],
				version: 2,
			}),
		).toEqual(
			expect.arrayContaining([
				{
					key: "media_processing_error_version_mismatch",
					values: { version: 1 },
				},
				{
					key: "media_processing_error_no_enabled_processors",
				},
			]),
		);
	});
});
