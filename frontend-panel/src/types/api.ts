// Re-export generated types for convenience
import type { components } from "@/services/api.generated";

// Schema types
export type UserInfo = components["schemas"]["UserInfo"];
export type MeResponse = components["schemas"]["MeResponse"];
export type FileInfo = components["schemas"]["FileInfo"];
export type FolderInfo = components["schemas"]["FolderInfo"];
export type FileListItem = components["schemas"]["FileListItem"];
export type FolderListItem = components["schemas"]["FolderListItem"];
export type StoragePolicy = components["schemas"]["StoragePolicy"];
export type SystemConfig = components["schemas"]["SystemConfig"];
export type FolderContents = components["schemas"]["FolderContents"];
export type FolderAncestorItem = components["schemas"]["FolderAncestorItem"];
export type ErrorCode = components["schemas"]["ErrorCode"];
export type UserRole = components["schemas"]["UserRole"];
export type UserStatus = components["schemas"]["UserStatus"];
export type DriverType = components["schemas"]["DriverType"];
export type ShareInfo = components["schemas"]["ShareInfo"];
export type MyShareInfo = components["schemas"]["MyShareInfo"];
export type ShareStatus = components["schemas"]["ShareStatus"];
export type SharePublicInfo = components["schemas"]["SharePublicInfo"];
export type UserStoragePolicy = components["schemas"]["UserStoragePolicy"];
export type WebdavAccount = components["schemas"]["WebdavAccount"];
export type WebdavAccountCreated =
	components["schemas"]["WebdavAccountCreated"];
export type WebdavAccountInfo = components["schemas"]["WebdavAccountInfo"];

export type TrashFileItem = components["schemas"]["TrashFileItem"];
export type TrashFolderItem = components["schemas"]["TrashFolderItem"];
export type TrashContents = components["schemas"]["TrashContents"];

// User preferences
export type UserPreferences = components["schemas"]["UserPreferences"];
export type UpdatePreferencesRequest =
	components["schemas"]["UpdatePreferencesReq"];

export type TrashItem =
	| (TrashFileItem & { entity_type: "file" })
	| (TrashFolderItem & { entity_type: "folder" });

export type FileVersion = components["schemas"]["FileVersion"];
export type UploadMode = components["schemas"]["UploadMode"];
export type UploadSessionStatus = components["schemas"]["UploadSessionStatus"];
export type InitUploadResponse = components["schemas"]["InitUploadResponse"];
export type ChunkUploadResponse = components["schemas"]["ChunkUploadResponse"];
export type UploadProgressResponse =
	components["schemas"]["UploadProgressResponse"];
export type CompletedPart = components["schemas"]["CompletedPartReq"];

// Auth
export type CheckResp = components["schemas"]["CheckResp"];
export type CreateUserReq = components["schemas"]["CreateUserReq"];

// Search
export type FileSearchItem = components["schemas"]["FileSearchItem"];
export type SearchParams = components["schemas"]["SearchParams"];
export type SearchResults = components["schemas"]["SearchResults"];

// Batch
export type BatchResult = components["schemas"]["BatchResult"];
export type BatchItemError = components["schemas"]["BatchItemError"];

// Audit
export type AuditLogEntry = components["schemas"]["AuditLogEntry"];
export type AuditLogPage = components["schemas"]["OffsetPage_AuditLogEntry"];
export type UserPage = components["schemas"]["OffsetPage_UserInfo"];
export type StoragePolicyPage =
	components["schemas"]["OffsetPage_StoragePolicy"];
export type UserStoragePolicyPage =
	components["schemas"]["OffsetPage_UserStoragePolicy"];
export type SharePage = components["schemas"]["OffsetPage_MyShareInfo"];
export type AdminSharePage = components["schemas"]["OffsetPage_ShareInfo"];
export type LockPage = components["schemas"]["OffsetPage_ResourceLock"];
export type SystemConfigPage = components["schemas"]["OffsetPage_SystemConfig"];
export type WebdavAccountPage =
	components["schemas"]["OffsetPage_WebdavAccountInfo"];

// ErrorCode constants (generated type is a union of literal numbers, we need named constants)
export const ErrorCode = {
	Success: 0,
	BadRequest: 1000,
	NotFound: 1001,
	InternalServerError: 1002,
	DatabaseError: 1003,
	ConfigError: 1004,
	EndpointNotFound: 1005,
	AuthFailed: 2000,
	TokenExpired: 2001,
	TokenInvalid: 2002,
	Forbidden: 2003,
	FileNotFound: 3000,
	FileTooLarge: 3001,
	FileTypeNotAllowed: 3002,
	FileUploadFailed: 3003,
	StoragePolicyNotFound: 4000,
	StorageDriverError: 4001,
	StorageQuotaExceeded: 4002,
	UnsupportedDriver: 4003,
	FolderNotFound: 5000,
	ShareNotFound: 6000,
	ShareExpired: 6001,
	SharePasswordRequired: 6002,
	ShareDownloadLimitReached: 6003,
	UploadAssemblyFailed: 3007,
	ThumbnailFailed: 3008,
	UploadAssembling: 3011,
} as const satisfies Record<string, ErrorCode>;

// API response wrapper
export interface ApiResponse<T> {
	code: ErrorCode;
	msg: string;
	data: T | null;
}
