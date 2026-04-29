import { toast } from "sonner";
import i18n from "@/i18n";
import { ApiError } from "@/services/http";
import {
	ErrorCode,
	type ErrorCode as ErrorCodeType,
} from "@/types/api-helpers";

const errorMessageKeys: Partial<Record<ErrorCodeType, string>> = {
	[ErrorCode.RateLimited]: "errors:rate_limited",
	[ErrorCode.MailNotConfigured]: "errors:mail_not_configured",
	[ErrorCode.MailDeliveryFailed]: "errors:mail_delivery_failed",
	[ErrorCode.AuthFailed]: "errors:auth_failed",
	[ErrorCode.TokenExpired]: "errors:token_expired",
	[ErrorCode.TokenInvalid]: "errors:token_invalid",
	[ErrorCode.Forbidden]: "errors:forbidden",
	[ErrorCode.PendingActivation]: "errors:pending_activation",
	[ErrorCode.ContactVerificationInvalid]: "errors:contact_verification_invalid",
	[ErrorCode.ContactVerificationExpired]: "errors:contact_verification_expired",
	[ErrorCode.FileNotFound]: "errors:file_not_found",
	[ErrorCode.FileTooLarge]: "errors:file_too_large",
	[ErrorCode.FileTypeNotAllowed]: "errors:file_type_not_allowed",
	[ErrorCode.FileUploadFailed]: "errors:file_upload_failed",
	[ErrorCode.UploadSessionNotFound]: "errors:upload_session_not_found",
	[ErrorCode.UploadSessionExpired]: "errors:upload_session_expired",
	[ErrorCode.ChunkUploadFailed]: "errors:chunk_upload_failed",
	[ErrorCode.ResourceLocked]: "errors:resource_locked",
	[ErrorCode.PreconditionFailed]: "errors:precondition_failed",
	[ErrorCode.UploadAssembling]: "errors:upload_assembling",
	[ErrorCode.StorageQuotaExceeded]: "errors:storage_quota_exceeded",
	[ErrorCode.StorageAuthFailed]: "errors:storage_auth_failed",
	[ErrorCode.StoragePermissionDenied]: "errors:storage_permission_denied",
	[ErrorCode.StorageMisconfigured]: "errors:storage_misconfigured",
	[ErrorCode.StorageObjectNotFound]: "errors:storage_not_found",
	[ErrorCode.StorageRateLimited]: "errors:storage_rate_limited",
	[ErrorCode.StorageTransientFailure]: "errors:storage_transient_failure",
	[ErrorCode.StoragePreconditionFailed]: "errors:storage_precondition_failed",
	[ErrorCode.StorageOperationUnsupported]:
		"errors:storage_operation_unsupported",
	[ErrorCode.FolderNotFound]: "errors:folder_not_found",
	[ErrorCode.ShareNotFound]: "errors:share_not_found",
	[ErrorCode.ShareExpired]: "errors:share_expired",
	[ErrorCode.SharePasswordRequired]: "errors:share_password_required",
	[ErrorCode.ShareDownloadLimitReached]: "errors:share_download_limit_reached",
};

const errorSubcodeKeys: Partial<Record<string, string>> = {
	"auth.username_exists": "errors:auth_username_exists",
	"auth.email_exists": "errors:auth_email_exists",
	"auth.identifier_exists": "errors:auth_identifier_exists",
	"file.etag_mismatch": "errors:file_etag_mismatch",
	"file.name_conflict": "errors:file_name_conflict",
	"folder.name_conflict": "errors:folder_name_conflict",
	"upload.field_read_failed": "errors:upload_field_read_failed",
	"upload.request_body_read_failed": "errors:upload_request_body_read_failed",
	"upload.request_body_size_overflow":
		"errors:upload_request_body_size_overflow",
	"upload.request_size_mismatch": "errors:upload_request_size_mismatch",
	"upload.temp_dir_create_failed": "errors:upload_temp_dir_create_failed",
	"upload.temp_file_create_failed": "errors:upload_temp_file_create_failed",
	"upload.temp_file_write_failed": "errors:upload_temp_file_write_failed",
	"upload.temp_file_flush_failed": "errors:upload_temp_file_flush_failed",
	"upload.local_staging_path_resolve_failed":
		"errors:upload_local_staging_path_resolve_failed",
	"upload.local_staging_dir_create_failed":
		"errors:upload_local_staging_dir_create_failed",
	"upload.local_staging_file_create_failed":
		"errors:upload_local_staging_file_create_failed",
	"upload.local_staging_write_failed":
		"errors:upload_local_staging_write_failed",
	"upload.local_staging_flush_failed":
		"errors:upload_local_staging_flush_failed",
	"upload.body_size_overflow": "errors:upload_body_size_overflow",
	"upload.empty_file": "errors:upload_empty_file",
	"upload.direct_relay_write_failed": "errors:upload_direct_relay_write_failed",
	"upload.direct_relay_shutdown_failed":
		"errors:upload_direct_relay_shutdown_failed",
	"upload.direct_relay_task_failed": "errors:upload_direct_relay_task_failed",
	"upload.declared_size_invalid": "errors:upload_declared_size_invalid",
	"upload.hash_temp_open_failed": "errors:upload_hash_temp_open_failed",
	"upload.hash_temp_read_failed": "errors:upload_hash_temp_read_failed",
	"upload.chunk_transport_mismatch": "errors:upload_chunk_transport_mismatch",
	"upload.chunk_session_invalid": "errors:upload_chunk_session_invalid",
	"upload.chunk_number_out_of_range": "errors:upload_chunk_number_out_of_range",
	"upload.chunk_size_mismatch": "errors:upload_chunk_size_mismatch",
	"upload.chunk_persist_failed": "errors:upload_chunk_persist_failed",
	"upload.status_conflict": "errors:upload_status_conflict",
	"upload.completed_file_missing": "errors:upload_completed_file_missing",
	"upload.previous_failure": "errors:upload_previous_failure",
	"upload.parts_required": "errors:upload_parts_required",
	"upload.incomplete_chunks": "errors:upload_incomplete_chunks",
	"upload.incomplete_parts": "errors:upload_incomplete_parts",
	"upload.missing_part": "errors:upload_missing_part",
	"upload.temp_object_missing": "errors:upload_temp_object_missing",
	"upload.temp_object_size_mismatch": "errors:upload_temp_object_size_mismatch",
	"upload.session_corrupted": "errors:upload_session_corrupted",
	"upload.chunk_relay_failed": "errors:upload_chunk_relay_failed",
	"upload.assembly_io_failed": "errors:upload_assembly_io_failed",
	"upload.assembly_size_overflow": "errors:upload_assembly_size_overflow",
	"storage.auth": "errors:storage_auth_failed",
	"storage.permission": "errors:storage_permission_denied",
	"storage.misconfigured": "errors:storage_misconfigured",
	"storage.not_found": "errors:storage_not_found",
	"storage.rate_limited": "errors:storage_rate_limited",
	"storage.transient": "errors:storage_transient_failure",
	"storage.precondition": "errors:storage_precondition_failed",
	"storage.unsupported": "errors:storage_operation_unsupported",
	"task.lease_lost": "errors:task_lease_lost",
	"task.lease_renewal_timed_out": "errors:task_lease_renewal_timed_out",
	"thumbnail.format_guess_failed": "errors:thumbnail_format_guess_failed",
	"thumbnail.decode_failed": "errors:thumbnail_decode_failed",
	"thumbnail.encode_failed": "errors:thumbnail_encode_failed",
	"thumbnail.processor_unavailable": "errors:thumbnail_processor_unavailable",
	"thumbnail.render_failed": "errors:thumbnail_render_failed",
	"thumbnail.output_invalid": "errors:thumbnail_output_invalid",
	"thumbnail.task_panicked": "errors:thumbnail_task_panicked",
	"thumbnail.source_too_large": "errors:thumbnail_source_too_large",
	"thumbnail.source_temp_create_failed":
		"errors:thumbnail_source_temp_create_failed",
	"thumbnail.source_stream_failed": "errors:thumbnail_source_stream_failed",
	"thumbnail.source_temp_flush_failed":
		"errors:thumbnail_source_temp_flush_failed",
	"thumbnail.source_temp_copy_failed":
		"errors:thumbnail_source_temp_copy_failed",
	"avatar.file_required": "errors:avatar_file_required",
	"avatar.upload_read_failed": "errors:avatar_upload_read_failed",
	"avatar.processor_unavailable": "errors:avatar_processor_unavailable",
	"avatar.empty_image": "errors:avatar_empty_image",
	"avatar.render_failed": "errors:avatar_render_failed",
	"avatar.output_invalid": "errors:avatar_output_invalid",
	"master_binding.disabled": "errors:master_binding_disabled",
	"managed_ingress.binding_mismatch": "errors:managed_ingress_binding_mismatch",
	"managed_ingress.default_delete_requires_replacement":
		"errors:managed_ingress_default_delete_requires_replacement",
	"managed_ingress.default_error": "errors:managed_ingress_default_error",
	"managed_ingress.default_missing": "errors:managed_ingress_default_missing",
	"managed_ingress.default_not_applied":
		"errors:managed_ingress_default_not_applied",
	"managed_ingress.required": "errors:managed_ingress_required",
	"managed_ingress.default_update_requires_replacement":
		"errors:managed_ingress_default_update_requires_replacement",
	"managed_ingress.driver_unsupported":
		"errors:managed_ingress_driver_unsupported",
	"managed_ingress.local_path_invalid":
		"errors:managed_ingress_local_path_invalid",
	"managed_ingress.single_primary_required":
		"errors:managed_ingress_single_primary_required",
	"remote_node.disabled": "errors:remote_node_disabled",
	"team.member_exists": "errors:team_member_exists",
	"webdav.username_exists": "errors:webdav_username_exists",
	"wopi.max_expected_size_exceeded": "errors:wopi_max_expected_size_exceeded",
	"remote_node.unique_conflict": "errors:remote_node_unique_conflict",
};

export function getApiErrorMessage(error: unknown) {
	if (error instanceof ApiError) {
		const key =
			(error.subcode ? errorSubcodeKeys[error.subcode] : undefined) ??
			errorMessageKeys[error.code];
		return key ? i18n.t(key) : error.message;
	}

	if (error instanceof Error) {
		return error.message;
	}

	return i18n.t("errors:unexpected_error");
}

export function handleApiError(error: unknown) {
	toast.error(getApiErrorMessage(error));
}
