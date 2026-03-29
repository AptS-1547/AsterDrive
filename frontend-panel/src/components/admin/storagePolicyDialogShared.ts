import {
	isPublicR2DevUrl,
	normalizeS3ConnectionFields,
} from "@/lib/s3Endpoint";
import type { DriverType, StoragePolicy } from "@/types/api";

export type S3UploadStrategy = "proxy_tempfile" | "relay_stream" | "presigned";

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
	s3_upload_strategy: S3UploadStrategy;
}

interface PolicyOptions {
	presigned_upload?: boolean;
	s3_upload_strategy?: S3UploadStrategy;
}

export function isS3UploadStrategy(value: unknown): value is S3UploadStrategy {
	return (
		value === "proxy_tempfile" ||
		value === "relay_stream" ||
		value === "presigned"
	);
}

export function parsePolicyOptions(options: string): PolicyOptions {
	try {
		const parsed = JSON.parse(options) as {
			presigned_upload?: unknown;
			s3_upload_strategy?: unknown;
		};
		return {
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
	return options.presigned_upload ? "presigned" : "proxy_tempfile";
}

export function buildPolicyOptions(form: PolicyFormData): string {
	if (form.driver_type !== "s3") {
		return JSON.stringify({});
	}

	return JSON.stringify({
		s3_upload_strategy: form.s3_upload_strategy,
	});
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
	s3_upload_strategy: "proxy_tempfile",
};
