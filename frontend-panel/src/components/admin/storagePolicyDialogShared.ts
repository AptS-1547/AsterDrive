import {
	isPublicR2DevUrl,
	normalizeS3ConnectionFields,
} from "@/lib/s3Endpoint";
import type {
	CreatePolicyRequest,
	DriverType,
	StoragePolicy,
	UpdatePolicyRequest,
} from "@/types/api";

export type S3UploadStrategy = "relay_stream" | "presigned";

export interface PolicyFormData {
	name: string;
	driver_type: DriverType;
	endpoint: string;
	bucket: string;
	access_key: string;
	secret_key: string;
	base_path: string;
	max_file_size: string;
	chunk_size: string;
	is_default: boolean;
	content_dedup: boolean;
	s3_upload_strategy: S3UploadStrategy;
}

interface PolicyOptions {
	content_dedup?: boolean;
	presigned_upload?: boolean;
	s3_upload_strategy?: S3UploadStrategy;
}

export function isS3UploadStrategy(value: unknown): value is S3UploadStrategy {
	return value === "relay_stream" || value === "presigned";
}

export function parsePolicyOptions(options: string): PolicyOptions {
	try {
		const parsed = JSON.parse(options) as {
			content_dedup?: unknown;
			presigned_upload?: unknown;
			s3_upload_strategy?: unknown;
		};
		return {
			content_dedup:
				typeof parsed.content_dedup === "boolean"
					? parsed.content_dedup
					: undefined,
			presigned_upload:
				typeof parsed.presigned_upload === "boolean"
					? parsed.presigned_upload
					: undefined,
			s3_upload_strategy: isS3UploadStrategy(parsed.s3_upload_strategy)
				? parsed.s3_upload_strategy
				: undefined,
		};
	} catch {
		return {};
	}
}

export function getEffectiveS3UploadStrategy(
	options: PolicyOptions,
): S3UploadStrategy {
	if (options.s3_upload_strategy) {
		return options.s3_upload_strategy;
	}
	return options.presigned_upload ? "presigned" : "relay_stream";
}

export function buildPolicyOptions(form: PolicyFormData): string {
	if (form.driver_type === "local") {
		return JSON.stringify(form.content_dedup ? { content_dedup: true } : {});
	}

	return JSON.stringify({
		s3_upload_strategy: form.s3_upload_strategy,
	});
}

export function getPolicyForm(policy: StoragePolicy): PolicyFormData {
	const options = parsePolicyOptions(policy.options);

	return {
		name: policy.name,
		driver_type: policy.driver_type,
		endpoint: policy.endpoint,
		bucket: policy.bucket,
		access_key: "",
		secret_key: "",
		base_path: policy.base_path,
		max_file_size:
			policy.max_file_size != null ? String(policy.max_file_size) : "",
		chunk_size:
			policy.chunk_size != null
				? String(Math.round(policy.chunk_size / 1024 / 1024))
				: "5",
		is_default: policy.is_default,
		content_dedup:
			policy.driver_type === "local" && options.content_dedup === true,
		s3_upload_strategy: getEffectiveS3UploadStrategy(options),
	};
}

export function normalizePolicyForm(form: PolicyFormData): PolicyFormData {
	if (form.driver_type !== "s3") {
		return form;
	}

	const normalized = normalizeS3ConnectionFields(form.endpoint, form.bucket);
	if (
		normalized.endpoint === form.endpoint &&
		normalized.bucket === form.bucket
	) {
		return form;
	}

	return {
		...form,
		endpoint: normalized.endpoint,
		bucket: normalized.bucket,
	};
}

export function buildPolicyTestPayload(form: PolicyFormData) {
	const normalizedForm = normalizePolicyForm(form);

	return {
		driver_type: normalizedForm.driver_type,
		endpoint: normalizedForm.endpoint || undefined,
		bucket: normalizedForm.bucket || undefined,
		access_key: normalizedForm.access_key || undefined,
		secret_key: normalizedForm.secret_key || undefined,
		base_path: normalizedForm.base_path || undefined,
	};
}

export function buildCreatePolicyPayload(
	form: PolicyFormData,
): CreatePolicyRequest {
	const normalizedForm = normalizePolicyForm(form);

	return {
		name: normalizedForm.name,
		driver_type: normalizedForm.driver_type,
		endpoint: normalizedForm.endpoint,
		bucket: normalizedForm.bucket,
		access_key: normalizedForm.access_key,
		secret_key: normalizedForm.secret_key,
		base_path: normalizedForm.base_path,
		max_file_size: normalizedForm.max_file_size
			? Number(normalizedForm.max_file_size)
			: undefined,
		chunk_size: normalizedForm.chunk_size
			? Number(normalizedForm.chunk_size) * 1024 * 1024
			: 0,
		is_default: normalizedForm.is_default,
		options: buildPolicyOptions(normalizedForm),
	};
}

export function buildUpdatePolicyPayload(
	form: PolicyFormData,
): UpdatePolicyRequest {
	const normalizedForm = normalizePolicyForm(form);
	const payload: UpdatePolicyRequest = {
		name: normalizedForm.name,
		endpoint: normalizedForm.endpoint,
		bucket: normalizedForm.bucket,
		base_path: normalizedForm.base_path,
		max_file_size: normalizedForm.max_file_size
			? Number(normalizedForm.max_file_size)
			: undefined,
		chunk_size: normalizedForm.chunk_size
			? Number(normalizedForm.chunk_size) * 1024 * 1024
			: 0,
		is_default: normalizedForm.is_default,
		options: buildPolicyOptions(normalizedForm),
	};

	if (normalizedForm.access_key) {
		payload.access_key = normalizedForm.access_key;
	}
	if (normalizedForm.secret_key) {
		payload.secret_key = normalizedForm.secret_key;
	}

	return payload;
}

export function hasConnectionFieldChanges(
	form: PolicyFormData,
	editingPolicy: StoragePolicy | null,
) {
	const normalizedForm = normalizePolicyForm(form);

	if (!editingPolicy) {
		return true;
	}

	if (normalizedForm.driver_type === "s3") {
		return (
			normalizedForm.endpoint !== editingPolicy.endpoint ||
			normalizedForm.bucket !== editingPolicy.bucket ||
			normalizedForm.base_path !== editingPolicy.base_path ||
			normalizedForm.access_key !== "" ||
			normalizedForm.secret_key !== ""
		);
	}

	return normalizedForm.base_path !== editingPolicy.base_path;
}

export function getS3ConnectionTestKey(form: PolicyFormData) {
	const normalizedForm = normalizePolicyForm(form);

	return JSON.stringify({
		driver_type: normalizedForm.driver_type,
		endpoint: normalizedForm.endpoint,
		bucket: normalizedForm.bucket,
		access_key: normalizedForm.access_key,
		secret_key: normalizedForm.secret_key,
		base_path: normalizedForm.base_path,
	});
}

export function getEndpointValidationMessage(
	form: PolicyFormData,
	t: (key: string) => string,
) {
	return form.driver_type === "s3" && isPublicR2DevUrl(form.endpoint)
		? t("s3_endpoint_public_r2_dev_error")
		: null;
}

export const emptyForm: PolicyFormData = {
	name: "",
	driver_type: "local",
	endpoint: "",
	bucket: "",
	access_key: "",
	secret_key: "",
	base_path: "",
	max_file_size: "",
	chunk_size: "5",
	is_default: false,
	content_dedup: false,
	s3_upload_strategy: "relay_stream",
};
