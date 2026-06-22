import { describe, expect, it } from "vitest";
import {
	resourceCacheKey,
	resourceCanonicalEtag,
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
});
