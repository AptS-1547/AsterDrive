import { toast } from "sonner";
import i18n from "@/i18n";
import { ApiError } from "@/services/http";
import {
	ErrorCode,
	type ErrorCode as ErrorCodeType,
} from "@/types/api-helpers";

const errorMessageKeys: Partial<Record<ErrorCodeType, string>> = {
	[ErrorCode.AuthFailed]: "errors:auth_failed",
	[ErrorCode.TokenExpired]: "errors:token_expired",
	[ErrorCode.TokenInvalid]: "errors:token_invalid",
	[ErrorCode.Forbidden]: "errors:forbidden",
	[ErrorCode.FileNotFound]: "errors:file_not_found",
	[ErrorCode.FileTooLarge]: "errors:file_too_large",
	[ErrorCode.FileTypeNotAllowed]: "errors:file_type_not_allowed",
	[ErrorCode.FileUploadFailed]: "errors:file_upload_failed",
	[ErrorCode.StorageQuotaExceeded]: "errors:storage_quota_exceeded",
	[ErrorCode.FolderNotFound]: "errors:folder_not_found",
	[ErrorCode.SharePasswordRequired]: "errors:share_password_required",
	[ErrorCode.ShareDownloadLimitReached]: "errors:share_download_limit_reached",
};

export function handleApiError(error: unknown) {
	if (error instanceof ApiError) {
		const key = errorMessageKeys[error.code];
		const message = key ? i18n.t(key) : error.message;
		toast.error(message);
	} else if (error instanceof Error) {
		toast.error(error.message);
	} else {
		toast.error(i18n.t("errors:unexpected_error"));
	}
}
