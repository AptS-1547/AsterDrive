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
	AuthFailed: 2000,
	TokenExpired: 2001,
	TokenInvalid: 2002,
	Forbidden: 2003,
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
	FolderNotFound: 5000,
	ShareNotFound: 6000,
	ShareExpired: 6001,
	SharePasswordRequired: 6002,
	ShareDownloadLimitReached: 6003,
} as const satisfies Record<string, ErrorCode>;

export interface ApiResponse<T> {
	code: ErrorCode;
	msg: string;
	data?: T | null;
}

export type TrashItem =
	| (TrashFileItem & { entity_type: "file" })
	| (TrashFolderItem & { entity_type: "folder" });
