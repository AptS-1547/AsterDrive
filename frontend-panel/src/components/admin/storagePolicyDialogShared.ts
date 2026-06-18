import { normalizeS3ConnectionFields } from "@/lib/s3Endpoint";
import type {
	CreatePolicyRequest,
	DriverType,
	ExecuteDraftStoragePolicyActionRequest,
	MicrosoftGraphCloud,
	OneDriveAccountMode,
	RemoteDownloadStrategy,
	RemoteUploadStrategy,
	S3DownloadStrategy,
	S3UploadStrategy,
	StorageConnectorAction,
	StorageConnectorActionKind,
	StorageConnectorDescriptor,
	StoragePolicy,
	StoragePolicyExecutableAction,
	StoragePolicyOptions,
	UpdatePolicyRequest,
} from "@/types/api";
import {
	buildStorageApplicationConfig,
	parseMicrosoftGraphScopes,
} from "./storage-policy-dialog/storagePolicyApplicationConfig";
import {
	buildPolicyOptions,
	getEffectiveRemoteDownloadStrategy,
	getEffectiveRemoteUploadStrategy,
	getEffectiveS3DownloadStrategy,
	getEffectiveS3PathStyle,
	getEffectiveS3UploadStrategy,
	normalizeThumbnailExtensions,
} from "./storage-policy-dialog/storagePolicyOptions";

export type {
	RemoteDownloadStrategy,
	RemoteUploadStrategy,
	S3DownloadStrategy,
	S3UploadStrategy,
} from "@/types/api";

export function isS3CompatibleDriver(driverType: DriverType) {
	return driverType === "s3" || driverType === "tencent_cos";
}

export function isObjectStorageDriver(driverType: DriverType) {
	return (
		driverType === "s3" ||
		driverType === "tencent_cos" ||
		driverType === "azure_blob"
	);
}

export function isOneDriveDriver(driverType: DriverType) {
	return driverType === "one_drive";
}

export function descriptorHasField(
	descriptor: StorageConnectorDescriptor | null | undefined,
	fieldName: string,
) {
	return descriptor?.fields.some((field) => field.name === fieldName) ?? false;
}

export function descriptorHasPolicyOptionField(
	descriptor: StorageConnectorDescriptor | null | undefined,
	fieldName: string,
) {
	return (
		descriptor?.fields.some(
			(field) => field.scope === "policy_options" && field.name === fieldName,
		) ?? false
	);
}

export function descriptorHasConnectionField(
	descriptor: StorageConnectorDescriptor | null | undefined,
	fieldName: string,
) {
	return (
		descriptor?.fields.some(
			(field) => field.scope === "connection" && field.name === fieldName,
		) ?? false
	);
}

export function supportsObjectStorageConnection(
	descriptor: StorageConnectorDescriptor | null | undefined,
) {
	return (
		descriptorHasConnectionField(descriptor, "endpoint") &&
		descriptorHasConnectionField(descriptor, "bucket") &&
		descriptorHasConnectionField(descriptor, "access_key") &&
		descriptorHasConnectionField(descriptor, "secret_key") &&
		descriptor?.upload_workflows.object_multipart_upload === true
	);
}

export function supportsRemoteNodeBinding(
	descriptor: StorageConnectorDescriptor | null | undefined,
) {
	return descriptor?.capabilities.remote_node_binding === true;
}

export function supportsS3TransferStrategy(
	descriptor: StorageConnectorDescriptor | null | undefined,
) {
	return descriptor?.capabilities.s3_transfer_strategy === true;
}

export function supportsOneDrivePolicyOptions(
	descriptor: StorageConnectorDescriptor | null | undefined,
) {
	return descriptorHasPolicyOptionField(descriptor, "account_mode");
}

export function supportsContentDedupPolicyOption(
	descriptor: StorageConnectorDescriptor | null | undefined,
) {
	return descriptorHasPolicyOptionField(descriptor, "content_dedup");
}

export function supportsApplicationCredentials(
	descriptor: StorageConnectorDescriptor | null | undefined,
) {
	return (
		descriptor?.fields.some(
			(field) => field.scope === "application_credential",
		) ?? false
	);
}

export function supportsStorageNativeProcessing(
	descriptor?: StorageConnectorDescriptor | null,
) {
	if (descriptor) {
		return (
			descriptor.capabilities.storage_native_thumbnail ||
			descriptor.capabilities.storage_native_media_metadata
		);
	}
	return false;
}

export function supportsDraftConnectionTest(
	descriptor?: StorageConnectorDescriptor | null,
) {
	return supportsStorageConnectorAction(
		descriptor,
		"test_draft_connection",
		"connection_test",
	);
}

export function supportsSavedConnectionTest(
	descriptor?: StorageConnectorDescriptor | null,
) {
	return supportsStorageConnectorAction(
		descriptor,
		"test_saved_connection",
		"connection_test",
	);
}

export function supportsStorageAuthorizationAction(
	descriptor?: StorageConnectorDescriptor | null,
) {
	return supportsStorageConnectorAction(
		descriptor,
		"start_authorization",
		"authorization",
	);
}

export function supportsCredentialValidationAction(
	descriptor?: StorageConnectorDescriptor | null,
) {
	return supportsStorageConnectorAction(
		descriptor,
		"validate_credential",
		"credential_validation",
	);
}

export function supportsStorageConnectorAction(
	descriptor: StorageConnectorDescriptor | null | undefined,
	action: StorageConnectorAction,
	kind?: StorageConnectorActionKind,
) {
	return descriptor
		? descriptor.actions.some(
				(actionDescriptor) =>
					actionDescriptor.action === action &&
					(kind === undefined || actionDescriptor.kind === kind),
			)
		: false;
}

export function supportsStoragePolicyAction(
	descriptor: StorageConnectorDescriptor | null | undefined,
	action: StoragePolicyExecutableAction,
) {
	return supportsStorageConnectorAction(descriptor, action, "policy_action");
}

export type S3CompatiblePromotionDriverType = Extract<
	DriverType,
	"tencent_cos"
>;

export interface S3CompatibleDriverPromotionTarget {
	driverLabel: string;
	driverType: S3CompatiblePromotionDriverType;
}

export function isTencentCosEndpoint(endpoint: string) {
	const trimmedEndpoint = endpoint.trim();
	if (!trimmedEndpoint) {
		return false;
	}

	try {
		const host = new URL(trimmedEndpoint).hostname.toLowerCase();
		return host === "myqcloud.com" || host.endsWith(".myqcloud.com");
	} catch {
		return false;
	}
}

export function getS3CompatibleDriverPromotionTarget(
	policy: {
		driver_type: DriverType;
		endpoint: string;
	} | null,
	getDriverLabel: (driverType: S3CompatiblePromotionDriverType) => string,
): S3CompatibleDriverPromotionTarget | null {
	if (policy?.driver_type !== "s3") {
		return null;
	}

	// Keep provider detection centralized so future OSS/OBS promotions only need
	// one UI registry change plus the matching backend allowlist entry.
	if (isTencentCosEndpoint(policy.endpoint)) {
		return {
			driverLabel: getDriverLabel("tencent_cos"),
			driverType: "tencent_cos",
		};
	}

	return null;
}

export const DEFAULT_STORAGE_NATIVE_THUMBNAIL_EXTENSIONS = [
	"jpg",
	"jpeg",
	"png",
	"webp",
	"gif",
];

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
	s3_path_style?: boolean;
	onedrive_cloud: MicrosoftGraphCloud;
	onedrive_account_mode: OneDriveAccountMode;
	onedrive_tenant: string;
	onedrive_drive_id: string;
	onedrive_root_item_id: string;
	onedrive_site_id: string;
	onedrive_group_id: string;
	onedrive_client_id: string;
	onedrive_client_secret: string;
	onedrive_scopes: string;
	storage_native_processing_enabled: boolean;
	storage_native_media_metadata_enabled?: boolean;
	thumbnail_processor: StoragePolicyOptions["thumbnail_processor"];
	thumbnail_extensions: string[];
	media_metadata_extensions?: string[];
}

function parseRemoteNodeId(value: string): number | undefined {
	if (!value) {
		return undefined;
	}

	const parsed = Number(value);
	return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : undefined;
}

export {
	buildPolicyOptions,
	getEffectiveRemoteDownloadStrategy,
	getEffectiveRemoteUploadStrategy,
	getEffectiveS3DownloadStrategy,
	getEffectiveS3PathStyle,
	getEffectiveS3UploadStrategy,
	normalizeThumbnailExtensions,
};

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
		...(policy.driver_type === "s3"
			? { s3_path_style: getEffectiveS3PathStyle(options) }
			: {}),
		onedrive_cloud: options.onedrive_cloud ?? "global",
		onedrive_account_mode: options.onedrive_account_mode ?? "work_or_school",
		onedrive_tenant: options.onedrive_tenant ?? "common",
		onedrive_drive_id: options.onedrive_drive_id ?? "",
		onedrive_root_item_id: options.onedrive_root_item_id ?? "",
		onedrive_site_id: options.onedrive_site_id ?? "",
		onedrive_group_id: options.onedrive_group_id ?? "",
		onedrive_client_id: "",
		onedrive_client_secret: "",
		onedrive_scopes: "",
		storage_native_processing_enabled:
			options.storage_native_processing_enabled === true,
		thumbnail_processor:
			options.storage_native_processing_enabled === true
				? (options.thumbnail_processor ?? null)
				: null,
		thumbnail_extensions:
			options.storage_native_processing_enabled === true
				? (options.thumbnail_extensions ?? [])
				: [],
		storage_native_media_metadata_enabled:
			options.storage_native_processing_enabled === true &&
			options.storage_native_media_metadata_enabled === true,
		media_metadata_extensions:
			options.storage_native_processing_enabled === true
				? (options.media_metadata_extensions ?? [])
				: [],
	};
}

export function normalizePolicyForm(form: PolicyFormData): PolicyFormData {
	if (
		!isObjectStorageDriver(form.driver_type) &&
		!isOneDriveDriver(form.driver_type)
	) {
		return form;
	}

	if (isOneDriveDriver(form.driver_type)) {
		const normalized = {
			...form,
			onedrive_tenant: form.onedrive_tenant.trim(),
			onedrive_drive_id: form.onedrive_drive_id.trim(),
			onedrive_root_item_id: form.onedrive_root_item_id.trim(),
			onedrive_site_id: form.onedrive_site_id.trim(),
			onedrive_group_id: form.onedrive_group_id.trim(),
			onedrive_client_id: form.onedrive_client_id.trim(),
			onedrive_client_secret: form.onedrive_client_secret.trim(),
			onedrive_scopes: form.onedrive_scopes.trim(),
		};
		return normalized.onedrive_tenant === form.onedrive_tenant &&
			normalized.onedrive_drive_id === form.onedrive_drive_id &&
			normalized.onedrive_root_item_id === form.onedrive_root_item_id &&
			normalized.onedrive_site_id === form.onedrive_site_id &&
			normalized.onedrive_group_id === form.onedrive_group_id &&
			normalized.onedrive_client_id === form.onedrive_client_id &&
			normalized.onedrive_client_secret === form.onedrive_client_secret &&
			normalized.onedrive_scopes === form.onedrive_scopes
			? form
			: normalized;
	}

	const normalized = normalizeS3ConnectionFields(form.endpoint, form.bucket);
	const normalizedAccessKey = form.access_key.trim();
	const normalizedSecretKey = form.secret_key.trim();
	if (
		normalized.endpoint === form.endpoint &&
		normalized.bucket === form.bucket &&
		normalizedAccessKey === form.access_key &&
		normalizedSecretKey === form.secret_key
	) {
		return form;
	}

	return {
		...form,
		endpoint: normalized.endpoint,
		bucket: normalized.bucket,
		access_key: normalizedAccessKey,
		secret_key: normalizedSecretKey,
	};
}

function getComparableOneDrivePolicyOptions(
	policy: StoragePolicy,
): StoragePolicyOptions {
	return buildPolicyOptions(getPolicyForm(policy));
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
		options: buildPolicyOptions(normalizedForm),
	};
}

export function buildTencentCosCorsPayload(
	form: PolicyFormData,
	policyId?: number | null,
): ExecuteDraftStoragePolicyActionRequest {
	const normalizedForm = normalizePolicyForm(form);

	return {
		action: "configure_tencent_cos_cors",
		policy_id: policyId ?? undefined,
		driver_type: normalizedForm.driver_type,
		endpoint: normalizedForm.endpoint || undefined,
		bucket: normalizedForm.bucket || undefined,
		access_key: normalizedForm.access_key || undefined,
		secret_key: normalizedForm.secret_key || undefined,
		base_path: normalizedForm.base_path || undefined,
		remote_node_id: parseRemoteNodeId(normalizedForm.remote_node_id),
		options: buildPolicyOptions(normalizedForm),
	};
}

export function buildCreatePolicyPayload(
	form: PolicyFormData,
): CreatePolicyRequest {
	const normalizedForm = normalizePolicyForm(form);
	const applicationConfig = buildStorageApplicationConfig(normalizedForm);
	const usesApplicationConfig = applicationConfig !== undefined;

	const payload: CreatePolicyRequest = {
		name: normalizedForm.name,
		driver_type: normalizedForm.driver_type,
		endpoint: normalizedForm.endpoint,
		bucket: normalizedForm.bucket,
		access_key: usesApplicationConfig ? "" : normalizedForm.access_key,
		secret_key: usesApplicationConfig ? "" : normalizedForm.secret_key,
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
	if (applicationConfig) {
		payload.application_config = applicationConfig;
	}
	return payload;
}

export function buildUpdatePolicyPayload(
	form: PolicyFormData,
): UpdatePolicyRequest {
	const normalizedForm = normalizePolicyForm(form);
	const applicationConfig = buildStorageApplicationConfig(normalizedForm);
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

	if (applicationConfig) {
		payload.application_config = applicationConfig;
	} else {
		if (normalizedForm.access_key) {
			payload.access_key = normalizedForm.access_key;
		}
		if (normalizedForm.secret_key) {
			payload.secret_key = normalizedForm.secret_key;
		}
	}

	return payload;
}

export { parseMicrosoftGraphScopes };

export function hasConnectionFieldChanges(
	form: PolicyFormData,
	editingPolicy: StoragePolicy | null,
) {
	const normalizedForm = normalizePolicyForm(form);

	if (!editingPolicy) {
		return true;
	}

	if (isObjectStorageDriver(normalizedForm.driver_type)) {
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

	if (isOneDriveDriver(normalizedForm.driver_type)) {
		return (
			normalizedForm.base_path !== editingPolicy.base_path ||
			JSON.stringify(buildPolicyOptions(normalizedForm)) !==
				JSON.stringify(getComparableOneDrivePolicyOptions(editingPolicy))
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
		options: buildPolicyOptions(normalizedForm),
	});
}

export function getEndpointValidationMessage(
	form: PolicyFormData,
	t: (key: string) => string,
	descriptor?: StorageConnectorDescriptor | null,
) {
	const shouldValidateEndpoint = descriptor
		? supportsObjectStorageConnection(descriptor)
		: isObjectStorageDriver(form.driver_type);
	if (!shouldValidateEndpoint) {
		return null;
	}

	const trimmedEndpoint = form.endpoint.trim();
	if (!trimmedEndpoint) {
		return null;
	}

	let endpointUrl: URL;
	try {
		endpointUrl = new URL(trimmedEndpoint);
	} catch {
		return form.driver_type === "azure_blob"
			? t("azure_blob_endpoint_protocol_required_error")
			: t("s3_endpoint_protocol_required_error");
	}

	if (endpointUrl.protocol !== "http:" && endpointUrl.protocol !== "https:") {
		return form.driver_type === "azure_blob"
			? t("azure_blob_endpoint_protocol_required_error")
			: t("s3_endpoint_protocol_required_error");
	}

	return null;
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
	s3_path_style: true,
	onedrive_cloud: "global",
	onedrive_account_mode: "work_or_school",
	onedrive_tenant: "common",
	onedrive_drive_id: "",
	onedrive_root_item_id: "",
	onedrive_site_id: "",
	onedrive_group_id: "",
	onedrive_client_id: "",
	onedrive_client_secret: "",
	onedrive_scopes: "",
	storage_native_processing_enabled: false,
	storage_native_media_metadata_enabled: false,
	thumbnail_processor: null,
	thumbnail_extensions: [],
	media_metadata_extensions: [],
};
