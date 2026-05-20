import { describe, expect, it } from "vitest";
import {
	getMediaProcessingConfigIssues,
	getMediaProcessingConfigIssuesFromString,
	MEDIA_PROCESSING_DEFAULT_FFMPEG_EXTENSIONS,
	MEDIA_PROCESSING_DEFAULT_FFPROBE_EXTENSIONS,
	MEDIA_PROCESSING_DEFAULT_LOFTY_EXTENSIONS,
	MEDIA_PROCESSING_DEFAULT_VIPS_EXTENSIONS,
	parseMediaProcessingConfig,
	serializeMediaProcessingConfig,
} from "@/components/admin/mediaProcessingConfigEditorShared";

describe("mediaProcessingConfigEditorShared", () => {
	it("parses and serializes fixed-order processor configs", () => {
		const draft = parseMediaProcessingConfig(`{
			"version": 2,
			"processors": [
				{
					"kind": "vips_cli",
					"enabled": true,
					"extensions": ["heic", ".heif"],
					"uses": ["thumbnail:image"]
				},
				{
					"kind": "images",
					"enabled": true,
					"uses": ["thumbnail:image", "metadata:image"]
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
					uses: ["thumbnail:image"],
				},
				{
					config: {
						command: "ffmpeg",
					},
					enabled: false,
					extensions: [...MEDIA_PROCESSING_DEFAULT_FFMPEG_EXTENSIONS],
					kind: "ffmpeg_cli",
					uses: ["thumbnail:video"],
				},
				{
					config: {
						command: "ffprobe",
					},
					enabled: false,
					extensions: [...MEDIA_PROCESSING_DEFAULT_FFPROBE_EXTENSIONS],
					kind: "ffprobe_cli",
					uses: ["metadata:video"],
				},
				{
					config: {
						command: "",
					},
					enabled: true,
					extensions: [...MEDIA_PROCESSING_DEFAULT_LOFTY_EXTENSIONS],
					kind: "lofty",
					uses: ["thumbnail:audio", "metadata:audio"],
				},
				{
					config: {
						command: "",
					},
					enabled: true,
					extensions: [],
					kind: "images",
					uses: ["thumbnail:image", "metadata:image"],
				},
			],
			version: 2,
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
					uses: ["thumbnail:image"],
				},
				{
					config: {
						command: "ffmpeg",
					},
					enabled: false,
					extensions: [...MEDIA_PROCESSING_DEFAULT_FFMPEG_EXTENSIONS],
					kind: "ffmpeg_cli",
					uses: ["thumbnail:video"],
				},
				{
					config: {
						command: "ffprobe",
					},
					enabled: false,
					extensions: [...MEDIA_PROCESSING_DEFAULT_FFPROBE_EXTENSIONS],
					kind: "ffprobe_cli",
					uses: ["metadata:video"],
				},
				{
					enabled: true,
					extensions: [...MEDIA_PROCESSING_DEFAULT_LOFTY_EXTENSIONS],
					kind: "lofty",
					uses: ["thumbnail:audio", "metadata:audio"],
				},
				{
					enabled: true,
					kind: "images",
					uses: ["thumbnail:image", "metadata:image"],
				},
			],
			version: 2,
		});
	});

	it("fills default vips extensions when the processor is missing from config", () => {
		const draft = parseMediaProcessingConfig(`{
			"version": 2,
			"processors": []
		}`);

		expect(draft.processors[0]).toEqual({
			config: {
				command: "vips",
			},
			enabled: false,
			extensions: [...MEDIA_PROCESSING_DEFAULT_VIPS_EXTENSIONS],
			kind: "vips_cli",
			uses: ["thumbnail:image"],
		});
		expect(draft.processors[1]).toEqual({
			config: {
				command: "ffmpeg",
			},
			enabled: false,
			extensions: [...MEDIA_PROCESSING_DEFAULT_FFMPEG_EXTENSIONS],
			kind: "ffmpeg_cli",
			uses: ["thumbnail:video"],
		});
	});

	it("backfills new default processor uses from older drafts", () => {
		const draft = parseMediaProcessingConfig(`{
			"version": 2,
			"processors": [
				{
					"kind": "lofty",
					"enabled": true,
					"extensions": ["mp3"],
					"uses": ["metadata:audio"]
				}
			]
		}`);

		expect(draft.processors[3]).toEqual({
			config: {
				command: "",
			},
			enabled: true,
			extensions: ["mp3"],
			kind: "lofty",
			uses: ["metadata:audio", "thumbnail:audio"],
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
						uses: ["thumbnail:image"],
					},
					{
						config: {
							command: "",
						},
						enabled: false,
						extensions: [],
						kind: "ffmpeg_cli",
						uses: ["thumbnail:video"],
					},
					{
						config: {
							command: "",
						},
						enabled: false,
						extensions: [],
						kind: "ffprobe_cli",
						uses: ["metadata:video"],
					},
					{
						config: {
							command: "",
						},
						enabled: false,
						extensions: [],
						kind: "lofty",
						uses: ["thumbnail:audio", "metadata:audio"],
					},
					{
						config: {
							command: "",
						},
						enabled: false,
						extensions: [],
						kind: "images",
						uses: ["thumbnail:image", "metadata:image"],
					},
				],
				version: 1,
			}),
		).toEqual(
			expect.arrayContaining([
				{
					key: "media_processing_error_version_mismatch",
					values: { version: 2 },
				},
				{
					key: "media_processing_error_no_enabled_processors",
				},
			]),
		);
	});
});
