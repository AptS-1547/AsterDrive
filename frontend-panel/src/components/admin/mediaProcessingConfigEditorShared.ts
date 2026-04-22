import type { MediaProcessorKind } from "@/types/api";

export const MEDIA_PROCESSING_CONFIG_KEY = "media_processing_registry_json";
export const MEDIA_PROCESSING_CONFIG_VERSION = 1;
export const MEDIA_PROCESSING_DEFAULT_VIPS_COMMAND = "vips";
export const MEDIA_PROCESSING_DEFAULT_VIPS_EXTENSIONS = [
	"csv",
	"mat",
	"img",
	"hdr",
	"pbm",
	"pgm",
	"ppm",
	"pfm",
	"pnm",
	"svg",
	"svgz",
	"j2k",
	"jp2",
	"jpt",
	"j2c",
	"jpc",
	"gif",
	"png",
	"jpg",
	"jpeg",
	"jpe",
	"webp",
	"tif",
	"tiff",
	"fits",
	"fit",
	"fts",
	"exr",
	"jxl",
	"pdf",
	"heic",
	"heif",
	"avif",
	"svs",
	"vms",
	"vmu",
	"ndpi",
	"scn",
	"mrxs",
	"svslide",
	"bif",
	"raw",
] as const;
export const MEDIA_PROCESSING_PROCESSOR_ORDER = [
	"vips_cli",
	"images",
] as const satisfies readonly MediaProcessorKind[];
export type MediaProcessingEditorProcessorKind =
	(typeof MEDIA_PROCESSING_PROCESSOR_ORDER)[number];

export interface MediaProcessingEditorProcessorConfig {
	command: string;
}

export interface MediaProcessingEditorProcessor {
	config: MediaProcessingEditorProcessorConfig;
	enabled: boolean;
	extensions: string[];
	kind: MediaProcessingEditorProcessorKind;
}

export interface MediaProcessingEditorConfig {
	processors: MediaProcessingEditorProcessor[];
	version: number;
}

export interface MediaProcessingValidationIssue {
	key: string;
	values?: Record<string, number | string>;
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function readString(value: unknown) {
	return typeof value === "string" ? value : "";
}

function readBoolean(value: unknown, fallback = false) {
	return typeof value === "boolean" ? value : fallback;
}

function readStringList(value: unknown) {
	if (!Array.isArray(value)) {
		return [];
	}

	return value
		.map((item) => readString(item).trim().replace(/^\./, "").toLowerCase())
		.filter(
			(item, index, items) => item.length > 0 && items.indexOf(item) === index,
		);
}

function readProcessorKind(
	value: unknown,
): MediaProcessingEditorProcessorKind | "" {
	const normalized = readString(value).trim().toLowerCase();
	if (normalized === "images" || normalized === "vips_cli") {
		return normalized;
	}
	return "";
}

function defaultEnabled(kind: MediaProcessorKind) {
	return kind === "images";
}

function createDefaultProcessor(
	kind: MediaProcessingEditorProcessorKind,
): MediaProcessingEditorProcessor {
	return {
		config: {
			command: kind === "vips_cli" ? MEDIA_PROCESSING_DEFAULT_VIPS_COMMAND : "",
		},
		enabled: defaultEnabled(kind),
		extensions:
			kind === "vips_cli" ? [...MEDIA_PROCESSING_DEFAULT_VIPS_EXTENSIONS] : [],
		kind,
	};
}

function normalizeProcessor(
	value: unknown,
): MediaProcessingEditorProcessor | null {
	if (!isRecord(value)) {
		return null;
	}

	const kind = readProcessorKind(value.kind);
	if (!kind) {
		return null;
	}

	const runtimeConfig = isRecord(value.config) ? value.config : undefined;

	return {
		config: {
			command:
				kind === "vips_cli"
					? readString(runtimeConfig?.command).trim() ||
						MEDIA_PROCESSING_DEFAULT_VIPS_COMMAND
					: "",
		},
		enabled: readBoolean(value.enabled, defaultEnabled(kind)),
		extensions: kind === "images" ? [] : readStringList(value.extensions),
		kind,
	};
}

function mergeProcessors(
	processors: MediaProcessingEditorProcessor[],
): MediaProcessingEditorProcessor[] {
	return MEDIA_PROCESSING_PROCESSOR_ORDER.map((kind) => {
		const matched = processors.find((processor) => processor.kind === kind);
		return matched ? { ...matched } : createDefaultProcessor(kind);
	});
}

export function parseMediaProcessingDelimitedInput(value: string) {
	return value
		.split(",")
		.map((item) => item.trim().replace(/^\./, "").toLowerCase())
		.filter(
			(item, index, items) => item.length > 0 && items.indexOf(item) === index,
		);
}

export function formatMediaProcessingDelimitedInput(values: string[]) {
	return values.join(", ");
}

export function parseMediaProcessingConfig(
	value: string,
): MediaProcessingEditorConfig {
	const parsed = JSON.parse(value) as unknown;
	if (!isRecord(parsed)) {
		throw new Error("media processing config must be an object");
	}

	const processors = Array.isArray(parsed.processors)
		? parsed.processors
				.map(normalizeProcessor)
				.filter((processor): processor is MediaProcessingEditorProcessor =>
					Boolean(processor),
				)
		: [];

	return {
		processors: mergeProcessors(processors),
		version:
			typeof parsed.version === "number"
				? parsed.version
				: MEDIA_PROCESSING_CONFIG_VERSION,
	};
}

export function serializeMediaProcessingConfig(
	config: MediaProcessingEditorConfig,
) {
	return JSON.stringify(
		{
			version: config.version,
			processors: mergeProcessors(config.processors).map((processor) => {
				const serialized = {
					enabled: processor.enabled,
					...(processor.kind !== "images" && processor.extensions.length > 0
						? { extensions: processor.extensions }
						: {}),
					kind: processor.kind,
				} as Record<string, unknown>;
				if (processor.kind === "vips_cli") {
					serialized.config = {
						command:
							processor.config.command.trim() ||
							MEDIA_PROCESSING_DEFAULT_VIPS_COMMAND,
					};
				}
				return serialized;
			}),
		},
		null,
		2,
	);
}

export function getMediaProcessingConfigIssues(
	config: MediaProcessingEditorConfig,
): MediaProcessingValidationIssue[] {
	const issues: MediaProcessingValidationIssue[] = [];

	if (config.version !== MEDIA_PROCESSING_CONFIG_VERSION) {
		issues.push({
			key: "media_processing_error_version_mismatch",
			values: { version: MEDIA_PROCESSING_CONFIG_VERSION },
		});
	}

	if (
		!mergeProcessors(config.processors).some((processor) => processor.enabled)
	) {
		issues.push({ key: "media_processing_error_no_enabled_processors" });
	}

	return issues;
}

export function getMediaProcessingConfigIssuesFromString(value: string) {
	try {
		return getMediaProcessingConfigIssues(parseMediaProcessingConfig(value));
	} catch {
		return [{ key: "media_processing_error_parse" }];
	}
}
