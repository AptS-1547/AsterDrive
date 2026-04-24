import type {
	ErrorCode as GeneratedErrorCode,
	TrashFileItem,
	TrashFolderItem,
} from "@/types/api";

export type ErrorCode = GeneratedErrorCode;

export const ErrorCode = {
	Success: 0,
	BadRequest: 1000,
	NotFound: 1001,
	InternalServerError: 1002,
	DatabaseError: 1003,
	ConfigError: 1004,
	EndpointNotFound: 1005,
	RateLimited: 1006,
	MailNotConfigured: 1007,
	MailDeliveryFailed: 1008,
	Conflict: 1009,
	AuthFailed: 2000,
	TokenExpired: 2001,
	TokenInvalid: 2002,
	Forbidden: 2003,
	PendingActivation: 2004,
	ContactVerificationInvalid: 2005,
	ContactVerificationExpired: 2006,
	FileNotFound: 3000,
	FileTooLarge: 3001,
	FileTypeNotAllowed: 3002,
	FileUploadFailed: 3003,
	UploadSessionNotFound: 3004,
	UploadSessionExpired: 3005,
	ChunkUploadFailed: 3006,
	UploadAssemblyFailed: 3007,
	ThumbnailFailed: 3008,
	ResourceLocked: 3009,
	PreconditionFailed: 3010,
	UploadAssembling: 3011,
	StoragePolicyNotFound: 4000,
	StorageDriverError: 4001,
	StorageQuotaExceeded: 4002,
	UnsupportedDriver: 4003,
	StorageAuthFailed: 4004,
	StoragePermissionDenied: 4005,
	StorageMisconfigured: 4006,
	StorageObjectNotFound: 4007,
	StorageRateLimited: 4008,
	StorageTransientFailure: 4009,
	StoragePreconditionFailed: 4010,
	StorageOperationUnsupported: 4011,
	FolderNotFound: 5000,
	ShareNotFound: 6000,
	ShareExpired: 6001,
	SharePasswordRequired: 6002,
	ShareDownloadLimitReached: 6003,
} as const satisfies Record<string, ErrorCode>;

export interface ApiErrorInfo {
	internal_code: string;
	subcode?: string | null;
	retryable?: boolean | null;
}

export interface ApiResponse<T> {
	code: ErrorCode;
	msg: string;
	data?: T | null;
	error?: ApiErrorInfo | null;
}

export type TrashItem =
	| (TrashFileItem & { entity_type: "file" })
	| (TrashFolderItem & { entity_type: "folder" });
