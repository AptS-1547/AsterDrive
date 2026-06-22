import { describe, expect, it, vi } from "vitest";
import {
	readyFileResourceFromRequest,
	resourceCacheKey,
	resourceCanonicalEtag,
	resourceConditionalHeaders,
	resourceCredentials,
	resourceRedirectPolicy,
	resourceRequestPath,
} from "@/lib/resourceRequest";

describe("resourceRequest", () => {
	it("uses plain string resources as both request path and cache key", () => {
		expect(resourceRequestPath("/files/1/download")).toBe("/files/1/download");
		expect(resourceCacheKey("/files/1/download")).toBe("/files/1/download");
		expect(resourceCanonicalEtag("/files/1/download")).toBeNull();
	});

	it("separates stable cache identity from temporary request paths", () => {
		const resource = {
			cacheKey: "/files/1/download",
			etag: '"etag-1"',
			requestPath: "/pv/token/file.txt",
		};

		expect(resourceRequestPath(resource)).toBe("/pv/token/file.txt");
		expect(resourceCacheKey(resource)).toBe("/files/1/download");
		expect(resourceCanonicalEtag(resource)).toBe('"etag-1"');
	});

	it("falls back to request path and null etag when optional fields are absent", () => {
		const resource = {
			requestPath: "/pv/token/file.txt",
		};

		expect(resourceRequestPath(resource)).toBe("/pv/token/file.txt");
		expect(resourceCacheKey(resource)).toBe("/pv/token/file.txt");
		expect(resourceCanonicalEtag(resource)).toBeNull();
		expect(
			resourceCanonicalEtag({
				etag: null,
				requestPath: "/pv/token/file.txt",
			}),
		).toBeNull();
	});

	it("defaults redirect policy to same-origin unless the resource declares otherwise", () => {
		expect(resourceRedirectPolicy("/files/1/download")).toBe(
			"same_origin_only",
		);
		expect(
			resourceRedirectPolicy({
				redirectPolicy: "may_cross_origin",
				requestPath: "/files/1/download",
			}),
		).toBe("may_cross_origin");
		expect(
			resourceRedirectPolicy({
				kind: "ready",
				identity: {
					cacheKey: "/files/1/download",
				},
				request: {
					conditionalHeaders: "forbidden",
					credentials: "include",
					redirectPolicy: "may_cross_origin",
					url: "/files/1/download?disposition=inline",
				},
				delivery: {
					mode: "direct_url",
				},
			}),
		).toBe("may_cross_origin");
	});

	it("uses request-level credential and conditional-header policy before falling back", () => {
		const fallback = vi.fn((path: string) => path.startsWith("/api/"));

		expect(resourceCredentials("/api/files/1/download", fallback)).toBe(true);
		expect(fallback).toHaveBeenCalledWith("/api/files/1/download");
		expect(
			resourceCredentials(
				{
					credentials: "omit",
					requestPath: "/api/files/1/download",
				},
				fallback,
			),
		).toBe(false);
		expect(
			resourceCredentials(
				{
					credentials: "include",
					requestPath: "https://cdn.example.com/file",
				},
				fallback,
			),
		).toBe(true);
		expect(
			resourceConditionalHeaders({
				conditionalHeaders: "forbidden",
				requestPath: "https://cdn.example.com/file",
			}),
		).toBe("forbidden");
		expect(resourceConditionalHeaders("/files/1/download")).toBe("allowed");
	});

	it("reads request policy from ready file resource handles", () => {
		const resource = {
			kind: "ready" as const,
			identity: {
				cacheKey: "/files/1/download",
				etag: '"etag-1"',
			},
			request: {
				conditionalHeaders: "forbidden" as const,
				credentials: "omit" as const,
				redirectPolicy: "may_cross_origin" as const,
				url: "https://cdn.example.com/file",
			},
			delivery: {
				mode: "direct_url" as const,
			},
		};

		expect(resourceRequestPath(resource)).toBe("https://cdn.example.com/file");
		expect(resourceCacheKey(resource)).toBe("/files/1/download");
		expect(resourceCanonicalEtag(resource)).toBe('"etag-1"');
		expect(resourceCredentials(resource, () => true)).toBe(false);
		expect(resourceConditionalHeaders(resource)).toBe("forbidden");
		expect(resourceRedirectPolicy(resource)).toBe("may_cross_origin");
	});

	it("promotes resource requests to ready handles with explicit or default policy", () => {
		expect(
			readyFileResourceFromRequest(
				{
					cacheKey: "/files/1/download",
					conditionalHeaders: "forbidden",
					credentials: "omit",
					deliveryMode: "media_stream",
					etag: '"etag-1"',
					redirectPolicy: "may_cross_origin",
					requestPath: "https://cdn.example.com/file",
				},
				"blob_url",
			),
		).toEqual({
			kind: "ready",
			identity: {
				cacheKey: "/files/1/download",
				etag: '"etag-1"',
			},
			request: {
				conditionalHeaders: "forbidden",
				credentials: "omit",
				redirectPolicy: "may_cross_origin",
				url: "https://cdn.example.com/file",
			},
			delivery: {
				mode: "media_stream",
			},
		});

		expect(
			readyFileResourceFromRequest(
				{ requestPath: "/files/2/download" },
				"text",
			),
		).toEqual({
			kind: "ready",
			identity: {
				cacheKey: "/files/2/download",
				etag: null,
			},
			request: {
				conditionalHeaders: "allowed",
				credentials: "include",
				redirectPolicy: "same_origin_only",
				url: "/files/2/download",
			},
			delivery: {
				mode: "text",
			},
		});
	});
});
