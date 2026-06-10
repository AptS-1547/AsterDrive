export interface NormalizedS3ConnectionFields {
	endpoint: string;
	bucket: string;
}

export function normalizeS3ConnectionFields(
	endpoint: string,
	bucket: string,
): NormalizedS3ConnectionFields {
	return {
		endpoint: endpoint.trim(),
		bucket: bucket.trim(),
	};
}
