import { useCallback, useEffect, useRef, useState } from "react";
import { shouldSendResourceCredentials } from "@/lib/apiUrl";
import { api } from "@/services/http";

interface TextCacheValue {
	content: string;
	etag: string | null;
}

interface TextCacheEntry extends Partial<TextCacheValue> {
	promise?: Promise<TextCacheValue>;
}

const textContentCache = new Map<string, TextCacheEntry>();
const textContentListeners = new Map<string, Set<() => void>>();

function subscribeTextContentInvalidation(path: string, listener: () => void) {
	let listeners = textContentListeners.get(path);
	if (!listeners) {
		listeners = new Set();
		textContentListeners.set(path, listeners);
	}
	listeners.add(listener);

	return () => {
		const current = textContentListeners.get(path);
		if (!current) return;
		current.delete(listener);
		if (current.size === 0) {
			textContentListeners.delete(path);
		}
	};
}

function notifyTextContentInvalidation(path?: string) {
	if (path) {
		for (const listener of textContentListeners.get(path) ?? []) {
			listener();
		}
		return;
	}

	const listeners = new Set<() => void>();
	for (const pathListeners of textContentListeners.values()) {
		for (const listener of pathListeners) {
			listeners.add(listener);
		}
	}
	for (const listener of listeners) {
		listener();
	}
}

async function fetchTextContent(
	path: string,
	force = false,
): Promise<TextCacheValue> {
	const cached = textContentCache.get(path);
	if (!force && cached?.promise) {
		return cached.promise;
	}
	const headers: Record<string, string> = {};
	if (!force && cached?.etag) {
		headers["If-None-Match"] = cached.etag;
	}

	const promise = api.client
		.get(path, {
			headers,
			responseType: "text",
			withCredentials: shouldSendResourceCredentials(path),
			validateStatus: (status) => status === 200 || status === 304,
		})
		.then((response) => {
			if (response.status === 304 && cached?.content !== undefined) {
				const next = {
					content: cached.content,
					etag: cached.etag ?? null,
				};
				textContentCache.set(path, next);
				return next;
			}
			const next = {
				content: response.data as string,
				etag: response.headers.etag ?? null,
			};
			textContentCache.set(path, next);
			return next;
		})
		.catch((error: unknown) => {
			if (cached?.content !== undefined) {
				textContentCache.set(path, {
					content: cached.content,
					etag: cached.etag ?? null,
				});
			} else {
				textContentCache.delete(path);
			}
			throw error;
		});

	textContentCache.set(path, {
		content: cached?.content,
		etag: cached?.etag ?? null,
		promise,
	});

	return promise;
}

export function invalidateTextContent(path?: string) {
	if (path) {
		textContentCache.delete(path);
		notifyTextContentInvalidation(path);
		return;
	}
	textContentCache.clear();
	notifyTextContentInvalidation();
}

export function clearTextContentCache() {
	textContentCache.clear();
}

export function useTextContent(path: string | null) {
	const [content, setContentState] = useState<string | null>(null);
	const [etag, setEtagState] = useState<string | null>(null);
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState(false);
	const requestIdRef = useRef(0);

	const setContent = useCallback(
		(value: string | null | ((prev: string | null) => string | null)) => {
			setContentState((prev) => {
				const next = typeof value === "function" ? value(prev) : value;
				if (path && next !== null) {
					const cached = textContentCache.get(path);
					textContentCache.set(path, {
						content: next,
						etag: cached?.etag ?? null,
						promise: cached?.promise,
					});
				}
				return next;
			});
		},
		[path],
	);

	const setEtag = useCallback(
		(value: string | null | ((prev: string | null) => string | null)) => {
			setEtagState((prev) => {
				const next = typeof value === "function" ? value(prev) : value;
				if (path) {
					const cached = textContentCache.get(path);
					if (cached?.content !== undefined) {
						textContentCache.set(path, {
							content: cached.content,
							etag: next,
							promise: cached.promise,
						});
					}
				}
				return next;
			});
		},
		[path],
	);

	const load = useCallback(
		async (force = false) => {
			requestIdRef.current += 1;
			const requestId = requestIdRef.current;
			if (!path) {
				setContentState(null);
				setEtagState(null);
				setLoading(false);
				setError(false);
				return;
			}

			const cached = textContentCache.get(path);
			if (cached?.content !== undefined) {
				setContentState(cached.content);
				setEtagState(cached.etag ?? null);
				setLoading(false);
				setError(false);
			}

			setLoading(cached?.content === undefined);
			setError(false);
			try {
				const next = await fetchTextContent(path, force);
				if (requestId !== requestIdRef.current) return;
				setContentState(next.content);
				setEtagState(next.etag);
			} catch {
				if (requestId !== requestIdRef.current) return;
				setError(true);
			} finally {
				if (requestId === requestIdRef.current) {
					setLoading(false);
				}
			}
		},
		[path],
	);

	const reload = useCallback(async () => {
		await load(true);
	}, [load]);

	useEffect(() => {
		if (!path) {
			void load();
			return;
		}

		const unsubscribe = subscribeTextContentInvalidation(path, () => {
			void load(true);
		});

		load();

		return () => {
			unsubscribe();
		};
	}, [load, path]);

	return {
		content,
		etag,
		loading,
		error,
		reload,
		setContent,
		setEtag,
	};
}
