import { formatDateTime } from "@/lib/format";
import { ApiError } from "@/services/http";
import type { ArchivePreviewManifest } from "@/types/api";
import { ApiErrorCode } from "@/types/api-helpers";
import type {
	ArchiveBreadcrumbItem,
	ArchiveBrowserEntry,
	ArchiveDirectoryEntry,
	ArchiveEntry,
	ArchivePreviewErrorKind,
} from "./archivePreviewTypes";

const archivePreviewDisabledCodes = new Set<string>([
	ApiErrorCode.ArchivePreviewDisabled,
	ApiErrorCode.ArchivePreviewUserDisabled,
	ApiErrorCode.ArchivePreviewShareDisabled,
]);

const archivePreviewRejectedCodes = new Set<string>([
	ApiErrorCode.ArchivePreviewRejected,
	ApiErrorCode.ArchivePreviewManifestTooLarge,
	ApiErrorCode.ArchivePreviewSourceSizeMismatch,
]);

function isArchiveFilenameEncodingError(message: string) {
	return message.includes("filename is not valid");
}

export function classifyArchivePreviewError(
	error: unknown,
): ArchivePreviewErrorKind {
	if (!(error instanceof ApiError)) {
		return "generic";
	}

	const message = error.message.toLowerCase();
	if (archivePreviewDisabledCodes.has(error.code)) {
		return "disabled";
	}
	if (error.code === ApiErrorCode.ArchivePreviewUnsupportedType) {
		return "unsupported";
	}
	if (error.code === ApiErrorCode.ArchivePreviewSourceTooLarge) {
		return "sourceTooLarge";
	}
	if (error.code === ApiErrorCode.ArchivePreviewInvalidArchive) {
		return "invalid";
	}
	if (archivePreviewRejectedCodes.has(error.code)) {
		if (isArchiveFilenameEncodingError(message)) {
			return "encoding";
		}
		return "rejected";
	}

	if (
		error.code === ApiErrorCode.BadRequest &&
		isArchiveFilenameEncodingError(message)
	) {
		return "encoding";
	}

	return "generic";
}

export function entryDepth(path: string) {
	return path.split("/").filter(Boolean).length - 1;
}

export function formatArchiveModifiedAt(value: string | null | undefined) {
	if (!value) return "";
	return formatDateTime(value);
}

export function parentPathForArchivePath(path: string) {
	const normalized = path.replace(/\/+$/u, "");
	const slash = normalized.lastIndexOf("/");
	if (slash < 0) return null;
	return normalized.slice(0, slash);
}

function fileNameForArchivePath(path: string) {
	const normalized = path.replace(/\/+$/u, "");
	const slash = normalized.lastIndexOf("/");
	return slash < 0 ? normalized : normalized.slice(slash + 1);
}

export function getArchiveEntryPath(entry: ArchiveBrowserEntry) {
	return entry.path.replace(/\/+$/u, "");
}

export function buildArchiveDirectoryEntries(
	entries: ArchiveEntry[],
): Map<string, ArchiveDirectoryEntry> {
	const directories = new Map<string, ArchiveDirectoryEntry>();

	const ensureDirectory = (path: string) => {
		const normalized = path.replace(/\/+$/u, "");
		if (!normalized || directories.has(normalized)) return;
		const parent = parentPathForArchivePath(normalized);
		if (parent) {
			ensureDirectory(parent);
		}
		directories.set(normalized, {
			path: normalized,
			name: fileNameForArchivePath(normalized),
			parent,
			kind: "directory",
			size: 0,
			compressed_size: 0,
			modified_at: null,
			synthetic: true,
		});
	};

	for (const entry of entries) {
		const normalizedPath = getArchiveEntryPath(entry);
		if (entry.kind === "directory") {
			ensureDirectory(normalizedPath);
		}
		const parent = entry.parent ?? parentPathForArchivePath(normalizedPath);
		if (parent) {
			ensureDirectory(parent);
		}
	}

	return directories;
}

export function displayParentForEntry(entry: ArchiveBrowserEntry) {
	return entry.parent ?? parentPathForArchivePath(getArchiveEntryPath(entry));
}

export function compareArchiveEntries(
	a: ArchiveBrowserEntry,
	b: ArchiveBrowserEntry,
) {
	if (a.kind !== b.kind) {
		return a.kind === "directory" ? -1 : 1;
	}
	return a.name.localeCompare(b.name, undefined, {
		numeric: true,
		sensitivity: "base",
	});
}

export function buildArchiveBreadcrumb(
	currentFolder: string | null,
	rootName: string,
): ArchiveBreadcrumbItem[] {
	const items: ArchiveBreadcrumbItem[] = [{ path: null, name: rootName }];
	if (!currentFolder) return items;

	const segments = currentFolder.split("/").filter(Boolean);
	let path = "";
	for (const segment of segments) {
		path = path ? `${path}/${segment}` : segment;
		items.push({ path, name: segment });
	}

	return items;
}

export function buildArchiveVisibleEntries(
	manifest: ArchivePreviewManifest,
	directoryEntries: Map<string, ArchiveDirectoryEntry>,
	query: string,
	currentFolder: string | null,
) {
	const normalized = query.trim().toLowerCase();
	const explicitDirectoryPaths = new Set<string>();
	for (const entry of manifest.entries) {
		if (entry.kind === "directory") {
			explicitDirectoryPaths.add(getArchiveEntryPath(entry));
		}
	}

	const entries: ArchiveBrowserEntry[] = [];
	for (const entry of directoryEntries.values()) {
		if (!explicitDirectoryPaths.has(entry.path)) {
			entries.push(entry);
		}
	}
	entries.push(...manifest.entries);

	if (normalized) {
		return entries
			.filter((entry) =>
				getArchiveEntryPath(entry).toLowerCase().includes(normalized),
			)
			.sort(compareArchiveEntries);
	}

	return entries
		.filter((entry) => displayParentForEntry(entry) === currentFolder)
		.sort(compareArchiveEntries);
}
