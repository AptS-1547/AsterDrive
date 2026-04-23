import {
	isPublicR2DevUrl,
	normalizeS3ConnectionFields,
} from "@/lib/s3Endpoint";
import type {
	CreatePolicyRequest,
	DriverType,
	RemoteDownloadStrategy,
	RemoteUploadStrategy,
	S3DownloadStrategy,
	S3UploadStrategy,
	StoragePolicy,
	StoragePolicyOptions,
	UpdatePolicyRequest,
} from "@/types/api";

export type {
	RemoteDownloadStrategy,
	RemoteUploadStrategy,
	S3DownloadStrategy,
	S3UploadStrategy,
} from "@/types/api";

export interface PolicyFormData {
	name: string;
	driver_type: DriverType;
	endpoint: string;
	bucket: string;
	access_key: string;
	secret_key: string;
	base_path: string;
	remote_node_id: string;
	max_file_size: string;
	chunk_size: string;
	is_default: boolean;
	content_dedup: boolean;
	remote_download_strategy: RemoteDownloadStrategy;
	remote_upload_strategy: RemoteUploadStrategy;
	s3_upload_strategy: S3UploadStrategy;
	s3_download_strategy: S3DownloadStrategy;
	thumbnail_processor: StoragePolicyOptions["thumbnail_processor"];
	thumbnail_extensions: string[];
}

function parseRemoteNodeId(value: string): number | undefined {
	if (!value) {
		return undefined;
	}

	const parsed = Number(value);
	return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : undefined;
}

export function getEffectiveS3UploadStrategy(
	options: StoragePolicyOptions,
): S3UploadStrategy {
	return options.s3_upload_strategy ?? "relay_stream";
}

export function getEffectiveS3DownloadStrategy(
	options: StoragePolicyOptions,
): S3DownloadStrategy {
	return options.s3_download_strategy ?? "relay_stream";
}

export function getEffectiveRemoteDownloadStrategy(
	options: StoragePolicyOptions,
): RemoteDownloadStrategy {
	return options.remote_download_strategy ?? "relay_stream";
}

export function getEffectiveRemoteUploadStrategy(
	options: StoragePolicyOptions,
): RemoteUploadStrategy {
	return options.remote_upload_strategy ?? "relay_stream";
}

export function buildPolicyOptions(form: PolicyFormData): StoragePolicyOptions {
	const options: StoragePolicyOptions = {};

	if (form.driver_type === "local") {
		if (form.content_dedup) {
			options.content_dedup = true;
		}
	} else if (form.driver_type === "remote") {
		Object.assign(options, {
			remote_download_strategy: form.remote_download_strategy,
			remote_upload_strategy: form.remote_upload_strategy,
		});
	} else {
		Object.assign(options, {
			s3_upload_strategy: form.s3_upload_strategy,
			s3_download_strategy: form.s3_download_strategy,
		});
	}

	if (form.thumbnail_processor) {
		options.thumbnail_processor = form.thumbnail_processor;
		options.thumbnail_extensions = form.thumbnail_extensions;
	}

	return options;
}

export function getPolicyForm(policy: StoragePolicy): PolicyFormData {
	const options = policy.options;

	return {
		name: policy.name,
		driver_type: policy.driver_type,
		endpoint: policy.endpoint,
		bucket: policy.bucket,
		access_key: "",
		secret_key: "",
		base_path: policy.base_path,
		remote_node_id:
			policy.remote_node_id != null ? String(policy.remote_node_id) : "",
		max_file_size:
			policy.max_file_size != null ? String(policy.max_file_size) : "",
		chunk_size:
			policy.chunk_size != null
				? String(Math.round(policy.chunk_size / 1024 / 1024))
				: "5",
		is_default: policy.is_default,
		content_dedup:
			policy.driver_type === "local" && options.content_dedup === true,
		remote_download_strategy: getEffectiveRemoteDownloadStrategy(options),
		remote_upload_strategy: getEffectiveRemoteUploadStrategy(options),
		s3_upload_strategy: getEffectiveS3UploadStrategy(options),
		s3_download_strategy: getEffectiveS3DownloadStrategy(options),
		thumbnail_processor: options.thumbnail_processor ?? null,
		thumbnail_extensions: options.thumbnail_extensions ?? [],
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
		remote_node_id: parseRemoteNodeId(normalizedForm.remote_node_id),
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
		remote_node_id: parseRemoteNodeId(normalizedForm.remote_node_id),
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
		remote_node_id: parseRemoteNodeId(normalizedForm.remote_node_id),
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

	if (normalizedForm.driver_type === "remote") {
		return (
			parseRemoteNodeId(normalizedForm.remote_node_id) !==
				editingPolicy.remote_node_id ||
			normalizedForm.base_path !== editingPolicy.base_path
		);
	}

	return normalizedForm.base_path !== editingPolicy.base_path;
}

export function getPolicyConnectionTestKey(form: PolicyFormData) {
	const normalizedForm = normalizePolicyForm(form);

	return JSON.stringify({
		driver_type: normalizedForm.driver_type,
		endpoint: normalizedForm.endpoint,
		bucket: normalizedForm.bucket,
		access_key: normalizedForm.access_key,
		secret_key: normalizedForm.secret_key,
		base_path: normalizedForm.base_path,
		remote_node_id: parseRemoteNodeId(normalizedForm.remote_node_id),
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
	remote_node_id: "",
	max_file_size: "",
	chunk_size: "5",
	is_default: false,
	content_dedup: false,
	remote_download_strategy: "relay_stream",
	remote_upload_strategy: "relay_stream",
	s3_upload_strategy: "relay_stream",
	s3_download_strategy: "relay_stream",
	thumbnail_processor: null,
	thumbnail_extensions: [],
};
