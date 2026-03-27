import { useEffect, useState } from "react";
import { logger } from "@/lib/logger";
import { api } from "@/services/http";

interface BlobCacheEntry {
	objectUrl?: string;
	etag?: string | null;
	promise?: Promise<string>;
	refCount: number;
	revokeTimer?: ReturnType<typeof setTimeout>;
}

const BLOB_URL_REVOKE_DELAY = 30_000;
const blobUrlCache = new Map<string, BlobCacheEntry>();

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

async function acquireBlobUrl(path: string): Promise<string> {
	const cached = blobUrlCache.get(path);
	if (cached?.revokeTimer) {
		clearTimeout(cached.revokeTimer);
		cached.revokeTimer = undefined;
	}
	if (cached?.objectUrl && cached.refCount > 0) {
		cached.refCount += 1;
		return cached.objectUrl;
	}
	if (cached?.promise) {
		cached.refCount += 1;
		return cached.promise;
	}

	const entry: BlobCacheEntry = cached ?? { refCount: 0 };
	entry.refCount += 1;
	const previousObjectUrl = entry.objectUrl;
	const previousEtag = entry.etag ?? null;
	const headers: Record<string, string> = {};
	if (previousObjectUrl && previousEtag) {
		headers["If-None-Match"] = previousEtag;
	}

	const MAX_RETRIES = 5;

	const fetchWithRetry = async (attempt: number): Promise<string> => {
		const response = await api.client.get(path, {
			headers,
			responseType: "blob",
			validateStatus: (status) =>
				status === 200 || status === 304 || status === 202,
		});

		// 202 = 缩略图正在后台生成，稍后重试
		if (response.status === 202) {
			if (attempt >= MAX_RETRIES) return previousObjectUrl ?? "";
			const retryAfter = Number(response.headers["retry-after"]) || 2;
			await new Promise((r) => setTimeout(r, retryAfter * 1000));
			return fetchWithRetry(attempt + 1);
		}

		const current = blobUrlCache.get(path);
		if (!current) {
			if (response.status === 200) {
				return URL.createObjectURL(response.data);
			}
			return previousObjectUrl ?? "";
		}

		if (response.status === 304 && previousObjectUrl) {
			current.objectUrl = previousObjectUrl;
			current.etag = previousEtag;
			current.promise = undefined;
			return previousObjectUrl;
		}

		const objectUrl = URL.createObjectURL(response.data);
		current.objectUrl = objectUrl;
		current.etag = response.headers.etag ?? null;
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
			current.objectUrl = previousObjectUrl;
			current.etag = previousEtag;
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
		return;
	}
	for (const [cachePath, entry] of blobUrlCache.entries()) {
		revokeEntry(cachePath, entry);
	}
}

export function clearBlobUrlCache() {
	invalidateBlobUrl();
}

export function useBlobUrl(path: string | null) {
	const [blobUrl, setBlobUrl] = useState<string | null>(null);
	const [error, setError] = useState(false);
	const [loading, setLoading] = useState(false);
	const [retryCount, setRetryCount] = useState(0);

	const retry = () => {
		if (path) invalidateBlobUrl(path);
		setError(false);
		setRetryCount((n) => n + 1);
	};

	// biome-ignore lint/correctness/useExhaustiveDependencies: retryCount is an intentional re-fetch trigger
	useEffect(() => {
		setBlobUrl(null);
		setError(false);
		if (!path) {
			setLoading(false);
			return;
		}

		const cached = blobUrlCache.get(path);
		if (cached?.objectUrl) {
			setBlobUrl(cached.objectUrl);
			setLoading(false);
		}

		let cancelled = false;
		setLoading(cached?.objectUrl === undefined);
		acquireBlobUrl(path)
			.then((nextBlobUrl) => {
				if (cancelled) return;
				setBlobUrl(nextBlobUrl || null);
			})
			.catch(() => {
				if (cancelled) return;
				setError(true);
			})
			.finally(() => {
				if (cancelled) return;
				setLoading(false);
			});

		return () => {
			cancelled = true;
			releaseBlobUrl(path);
		};
	}, [path, retryCount]);

	return { blobUrl, error, loading, retry };
}
