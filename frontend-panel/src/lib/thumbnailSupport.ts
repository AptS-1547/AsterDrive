import type {
	PublicExtensionSupport,
	PublicThumbnailSupport,
} from "@/types/api";

export function getThumbnailExtension(fileName: string) {
	const trimmed = fileName.trim().toLowerCase();
	const dot = trimmed.lastIndexOf(".");
	if (dot <= 0 || dot === trimmed.length - 1) {
		return "";
	}
	return trimmed.slice(dot + 1);
}

export function supportsThumbnailExtension(
	fileName: string,
	extensions: string[] | undefined,
) {
	const extension = getThumbnailExtension(fileName);
	return supportsNormalizedThumbnailExtension(extension, extensions);
}

function supportsExtensionGroup(
	fileName: string,
	support?: PublicExtensionSupport,
) {
	return (
		support?.enabled === true &&
		supportsThumbnailExtension(fileName, support.extensions)
	);
}

export function supportsGeneratedThumbnailFile(
	fileName: string,
	config?: PublicThumbnailSupport | null,
) {
	return (
		supportsExtensionGroup(fileName, config?.image_thumbnail) ||
		supportsExtensionGroup(fileName, config?.audio_thumbnail) ||
		supportsExtensionGroup(fileName, config?.video_thumbnail)
	);
}

function supportsNormalizedThumbnailExtension(
	extension: string,
	extensions: string[] | undefined,
) {
	if (!extension || !extensions?.length) return false;
	return extensions.some(
		(candidate) => normalizeExtension(candidate) === extension,
	);
}

function normalizeExtension(value: string) {
	return value.trim().replace(/^\./, "").toLowerCase();
}

export function supportsImagePreviewExtension(
	fileName: string,
	extensions: string[] | undefined,
) {
	return supportsThumbnailExtension(fileName, extensions);
}

export function imagePreviewExtensionCandidatesFromMime(mimeType: string) {
	const mime = mimeType.trim().toLowerCase().split(";", 1)[0] ?? "";
	if (!mime.startsWith("image/")) return [];
	const subtype = mime.slice("image/".length);
	switch (subtype) {
		case "jpeg":
		case "pjpeg":
			return ["jpg", "jpeg", "jpe"];
		case "svg+xml":
			return ["svg"];
		case "tiff":
			return ["tif", "tiff"];
		case "x-icon":
		case "vnd.microsoft.icon":
			return ["ico"];
		default:
			return [subtype.replace(/^\.+/, "")].filter(Boolean);
	}
}

export function supportsImagePreviewFile(
	fileName: string,
	mimeType: string,
	extensions: string[] | undefined,
) {
	if (supportsImagePreviewExtension(fileName, extensions)) return true;
	return imagePreviewExtensionCandidatesFromMime(mimeType).some((extension) =>
		supportsNormalizedThumbnailExtension(extension, extensions),
	);
}
