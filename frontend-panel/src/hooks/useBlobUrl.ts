import { useEffect, useState } from "react";
import { logger } from "@/lib/logger";
import { api } from "@/services/http";

type BlobFetchLane = "default" | "thumbnail";

interface BlobCacheEntry {
	blob?: Blob;
	lane?: BlobFetchLane;
	missing?: boolean;
	objectUrl?: string;
	etag?: string | null;
	promise?: Promise<string>;
	refCount: number;
	revokeTimer?: ReturnType<typeof setTimeout>;
}

interface BlobUrlOptions {
	lane?: BlobFetchLane;
}

const BLOB_URL_REVOKE_DELAY = 30_000;
const BLOB_FETCH_LIMITS: Record<BlobFetchLane, number> = {
	default: 6,
	thumbnail: 1,
};
const blobUrlCache = new Map<string, BlobCacheEntry>();
const blobUrlListeners = new Map<string, Set<() => void>>();
const pendingBlobFetches: Record<BlobFetchLane, Array<() => void>> = {
	default: [],
	thumbnail: [],
};
const activeBlobFetches: Record<BlobFetchLane, number> = {
	default: 0,
	thumbnail: 0,
};

function shouldPersistBlobInSession(lane: BlobFetchLane) {
	return lane === "thumbnail";
}

function isMissingThumbnailResponse(status: number, lane: BlobFetchLane) {
	return lane === "thumbnail" && status === 404;
}

function scheduleBlobFetch<T>(lane: BlobFetchLane, task: () => Promise<T>) {
	return new Promise<T>((resolve, reject) => {
		const run = () => {
			activeBlobFetches[lane] += 1;
			task()
				.then(resolve, reject)
				.finally(() => {
					activeBlobFetches[lane] = Math.max(0, activeBlobFetches[lane] - 1);
					const next = pendingBlobFetches[lane].shift();
					next?.();
				});
		};

		if (activeBlobFetches[lane] < BLOB_FETCH_LIMITS[lane]) {
			run();
			return;
		}

		pendingBlobFetches[lane].push(run);
	});
}

function revokeEntry(path: string, entry: BlobCacheEntry) {
	if (entry.revokeTimer) {
		clearTimeout(entry.revokeTimer);
		entry.revokeTimer = undefined;
	}
	if (entry.objectUrl) {
		URL.revokeObjectURL(entry.objectUrl);
	}
	blobUrlCache.delete(path);
}

function subscribeBlobUrlInvalidation(path: string, listener: () => void) {
	let listeners = blobUrlListeners.get(path);
	if (!listeners) {
		listeners = new Set();
		blobUrlListeners.set(path, listeners);
	}
	listeners.add(listener);

	return () => {
		const current = blobUrlListeners.get(path);
		if (!current) return;
		current.delete(listener);
		if (current.size === 0) {
			blobUrlListeners.delete(path);
		}
	};
}

function notifyBlobUrlInvalidation(path?: string) {
	if (path) {
		for (const listener of blobUrlListeners.get(path) ?? []) {
			listener();
		}
		return;
	}

	const listeners = new Set<() => void>();
	for (const pathListeners of blobUrlListeners.values()) {
		for (const listener of pathListeners) {
			listeners.add(listener);
		}
	}
	for (const listener of listeners) {
		listener();
	}
}

async function acquireBlobUrl(
	path: string,
	lane: BlobFetchLane,
): Promise<string> {
	const cached = blobUrlCache.get(path);
	if (cached?.revokeTimer) {
		clearTimeout(cached.revokeTimer);
		cached.revokeTimer = undefined;
	}
	if (
		cached?.objectUrl &&
		(cached.refCount > 0 ||
			shouldPersistBlobInSession(cached.lane ?? lane) ||
			shouldPersistBlobInSession(lane))
	) {
		cached.lane = lane;
		cached.refCount += 1;
		return cached.objectUrl;
	}
	if (
		cached?.missing &&
		(cached.refCount > 0 ||
			shouldPersistBlobInSession(cached.lane ?? lane) ||
			shouldPersistBlobInSession(lane))
	) {
		cached.lane = lane;
		cached.refCount += 1;
		return "";
	}
	if (cached?.promise) {
		cached.lane = lane;
		cached.refCount += 1;
		return cached.promise;
	}

	const entry: BlobCacheEntry = cached ?? { refCount: 0 };
	entry.lane = lane;
	entry.refCount += 1;
	const previousBlob = entry.blob;
	const previousObjectUrl = entry.objectUrl;
	const previousEtag = entry.etag ?? null;
	const headers: Record<string, string> = {};
	if (previousObjectUrl && previousEtag) {
		headers["If-None-Match"] = previousEtag;
	}

	const MAX_RETRIES = 5;

	const fetchWithRetry = async (attempt: number): Promise<string> => {
		const response = await scheduleBlobFetch(lane, () =>
			api.client.get(path, {
				headers,
				responseType: "blob",
				validateStatus: (status) =>
					status === 200 ||
					status === 304 ||
					status === 202 ||
					isMissingThumbnailResponse(status, lane),
			}),
		);

		// 202 = 缩略图正在后台生成，稍后重试
		if (response.status === 202) {
			if (attempt >= MAX_RETRIES) return previousObjectUrl ?? "";
			const retryAfter = Number(response.headers["retry-after"]) || 2;
			await new Promise((r) => setTimeout(r, retryAfter * 1000));
			return fetchWithRetry(attempt + 1);
		}

		const current = blobUrlCache.get(path);
		if (!current) {
			if (isMissingThumbnailResponse(response.status, lane)) {
				return "";
			}
			if (response.status === 200) {
				return URL.createObjectURL(response.data);
			}
			return previousObjectUrl ?? "";
		}

		if (isMissingThumbnailResponse(response.status, lane)) {
			current.blob = undefined;
			current.objectUrl = undefined;
			current.etag = null;
			current.missing = true;
			current.promise = undefined;
			if (previousObjectUrl) {
				URL.revokeObjectURL(previousObjectUrl);
			}
			notifyBlobUrlInvalidation(path);
			return "";
		}

		if (response.status === 304 && previousObjectUrl) {
			current.objectUrl = previousObjectUrl;
			current.blob = previousBlob;
			current.etag = previousEtag;
			current.missing = false;
			current.promise = undefined;
			return previousObjectUrl;
		}

		const blob =
			response.data instanceof Blob
				? response.data
				: new Blob([response.data as BlobPart]);
		const objectUrl = URL.createObjectURL(blob);
		current.blob = blob;
		current.objectUrl = objectUrl;
		current.etag = response.headers.etag ?? null;
		current.missing = false;
		current.promise = undefined;
		if (previousObjectUrl && previousObjectUrl !== objectUrl) {
			URL.revokeObjectURL(previousObjectUrl);
		}
		return objectUrl;
	};

	const promise = fetchWithRetry(0).catch((error: unknown) => {
		logger.warn("blob fetch failed", path, error);
		const current = blobUrlCache.get(path);
		if (current) {
			current.promise = undefined;
			current.blob = previousBlob;
			current.objectUrl = previousObjectUrl;
			current.etag = previousEtag;
			current.missing = false;
			if (!current.objectUrl && current.refCount <= 0) {
				blobUrlCache.delete(path);
			}
		}
		throw error;
	});
	entry.promise = promise;
	blobUrlCache.set(path, entry);
	return promise;
}

function releaseBlobUrl(path: string) {
	const cached = blobUrlCache.get(path);
	if (!cached) return;
	cached.refCount = Math.max(0, cached.refCount - 1);
	if (cached.refCount > 0) return;
	if (shouldPersistBlobInSession(cached.lane ?? "default")) {
		if (cached.revokeTimer) {
			clearTimeout(cached.revokeTimer);
			cached.revokeTimer = undefined;
		}
		return;
	}
	if (cached.revokeTimer) clearTimeout(cached.revokeTimer);
	cached.revokeTimer = setTimeout(() => {
		const current = blobUrlCache.get(path);
		if (!current || current.refCount > 0) return;
		revokeEntry(path, current);
	}, BLOB_URL_REVOKE_DELAY);
}

export function invalidateBlobUrl(path?: string) {
	if (path) {
		const cached = blobUrlCache.get(path);
		if (cached) revokeEntry(path, cached);
		notifyBlobUrlInvalidation(path);
		return;
	}
	for (const [cachePath, entry] of blobUrlCache.entries()) {
		revokeEntry(cachePath, entry);
	}
	notifyBlobUrlInvalidation();
}

export function clearBlobUrlCache() {
	for (const [cachePath, entry] of blobUrlCache.entries()) {
		revokeEntry(cachePath, entry);
	}
}

export function useBlobUrl(path: string | null, options?: BlobUrlOptions) {
	const [blob, setBlob] = useState<Blob | null>(null);
	const [blobUrl, setBlobUrl] = useState<string | null>(null);
	const [error, setError] = useState(false);
	const [loading, setLoading] = useState(false);
	const [retryCount, setRetryCount] = useState(0);
	const lane = options?.lane ?? "default";

	const retry = () => {
		setError(false);
		if (path) {
			invalidateBlobUrl(path);
		}
	};

	// biome-ignore lint/correctness/useExhaustiveDependencies: retryCount is an intentional re-fetch trigger
	useEffect(() => {
		setBlob(null);
		setBlobUrl(null);
		setError(false);
		if (!path) {
			setLoading(false);
			return;
		}

		const unsubscribe = subscribeBlobUrlInvalidation(path, () => {
			setBlob(null);
			setBlobUrl(null);
			setError(false);
			setRetryCount((n) => n + 1);
		});

		const cached = blobUrlCache.get(path);
		if (cached?.objectUrl) {
			setBlob(cached.blob ?? null);
			setBlobUrl(cached.objectUrl);
			setLoading(false);
		}

		let cancelled = false;
		setLoading(cached?.objectUrl === undefined);
		acquireBlobUrl(path, lane)
			.then((nextBlobUrl) => {
				if (cancelled) return;
				setBlob(blobUrlCache.get(path)?.blob ?? null);
				setBlobUrl(nextBlobUrl || null);
			})
			.catch(() => {
				if (cancelled) return;
				setBlob(null);
				setError(true);
			})
			.finally(() => {
				if (cancelled) return;
				setLoading(false);
			});

		return () => {
			cancelled = true;
			unsubscribe();
			releaseBlobUrl(path);
		};
	}, [lane, path, retryCount]);

	return { blob, blobUrl, error, loading, retry };
}
