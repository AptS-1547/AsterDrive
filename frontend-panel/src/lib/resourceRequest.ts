export interface ResourceRequest {
	cacheKey?: string;
	etag?: string | null;
	requestPath: string;
}

export type ResourcePath = string | ResourceRequest;

export function resourceRequestPath(resource: ResourcePath) {
	return typeof resource === "string" ? resource : resource.requestPath;
}

export function resourceCacheKey(resource: ResourcePath) {
	return typeof resource === "string"
		? resource
		: (resource.cacheKey ?? resource.requestPath);
}

export function resourceCanonicalEtag(resource: ResourcePath) {
	return typeof resource === "string" ? null : (resource.etag ?? null);
}
