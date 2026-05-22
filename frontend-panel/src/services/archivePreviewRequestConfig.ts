import type { ArchiveFilenameEncoding } from "@/types/api";
import type { ApiRequestConfig } from "./http";

export type ArchivePreviewRequestOptions = Pick<ApiRequestConfig, "signal"> & {
	filenameEncoding?: ArchiveFilenameEncoding;
};

export function archivePreviewRequestConfig(
	options?: ArchivePreviewRequestOptions,
): ApiRequestConfig | undefined {
	if (!options?.signal && !options?.filenameEncoding) {
		return undefined;
	}
	return {
		...(options.signal ? { signal: options.signal } : {}),
		...(options.filenameEncoding
			? { params: { filename_encoding: options.filenameEncoding } }
			: {}),
	};
}
