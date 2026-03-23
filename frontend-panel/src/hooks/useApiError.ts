import { toast } from "sonner";
import i18n from "@/i18n";
import { ApiError } from "@/services/http";
import { ErrorCode } from "@/types/api";

const errorMessageKeys: Partial<Record<ErrorCode, string>> = {
	[ErrorCode.AuthFailed]: "common:auth_failed",
	[ErrorCode.TokenExpired]: "common:token_expired",
	[ErrorCode.TokenInvalid]: "common:token_invalid",
	[ErrorCode.Forbidden]: "common:forbidden",
	[ErrorCode.FileNotFound]: "common:file_not_found",
	[ErrorCode.FileTooLarge]: "common:file_too_large",
	[ErrorCode.FileTypeNotAllowed]: "common:file_type_not_allowed",
	[ErrorCode.FileUploadFailed]: "common:file_upload_failed",
	[ErrorCode.StorageQuotaExceeded]: "common:storage_quota_exceeded",
	[ErrorCode.FolderNotFound]: "common:folder_not_found",
	[ErrorCode.SharePasswordRequired]: "common:share_password_required",
	[ErrorCode.ShareDownloadLimitReached]: "common:share_download_limit_reached",
};

export function handleApiError(error: unknown) {
	if (error instanceof ApiError) {
		const key = errorMessageKeys[error.code];
		const message = key ? i18n.t(key) : error.message;
		toast.error(message);
	} else if (error instanceof Error) {
		toast.error(error.message);
	} else {
		toast.error(i18n.t("common:unexpected_error"));
	}
}
