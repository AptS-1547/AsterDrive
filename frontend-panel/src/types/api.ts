// Re-export generated types for convenience
import type { components } from "@/services/api.generated";

// Core responses
export type ErrorCode = components["schemas"]["ErrorCode"];
export type HealthResponse = components["schemas"]["HealthResponse"];
export type MemoryStatsResponse = components["schemas"]["MemoryStatsResponse"];

// Auth and user
export type AvatarInfo = components["schemas"]["AvatarInfo"];
export type AvatarSource = components["schemas"]["AvatarSource"];
export type CheckResp = components["schemas"]["CheckResp"];
export type CreateUserReq = components["schemas"]["CreateUserReq"];
export type MeResponse = components["schemas"]["MeResponse"];
export type UpdateAvatarSourceRequest =
	components["schemas"]["UpdateAvatarSourceReq"];
export type UpdatePreferencesRequest =
	components["schemas"]["UpdatePreferencesReq"];
export type UpdateProfileRequest = components["schemas"]["UpdateProfileReq"];
export type UserInfo = components["schemas"]["UserInfo"];
export type UserPage = components["schemas"]["OffsetPage_UserInfo"];
export type UserPreferences = components["schemas"]["UserPreferences"];
export type UserProfileInfo = components["schemas"]["UserProfileInfo"];
export type UserRole = components["schemas"]["UserRole"];
export type UserStatus = components["schemas"]["UserStatus"];
export type UserStoragePolicy = components["schemas"]["UserStoragePolicy"];
export type UserStoragePolicyPage =
	components["schemas"]["OffsetPage_UserStoragePolicy"];

// Files, folders, and trash
export type FileInfo = components["schemas"]["FileInfo"];
export type FileListItem = components["schemas"]["FileListItem"];
export type FileVersion = components["schemas"]["FileVersion"];
export type FolderAncestorItem = components["schemas"]["FolderAncestorItem"];
export type FolderContents = components["schemas"]["FolderContents"];
export type FolderInfo = components["schemas"]["FolderInfo"];
export type FolderListItem = components["schemas"]["FolderListItem"];
export type PurgedCountResponse = components["schemas"]["PurgedCountResponse"];
export type TrashContents = components["schemas"]["TrashContents"];
export type TrashFileItem = components["schemas"]["TrashFileItem"];
export type TrashFolderItem = components["schemas"]["TrashFolderItem"];
export type TrashItem =
	| (TrashFileItem & { entity_type: "file" })
	| (TrashFolderItem & { entity_type: "folder" });

// Sharing and search
export type AdminSharePage = components["schemas"]["OffsetPage_ShareInfo"];
export type FileSearchItem = components["schemas"]["FileSearchItem"];
export type MyShareInfo = components["schemas"]["MyShareInfo"];
export type SearchParams = components["schemas"]["SearchParams"];
export type SearchResults = components["schemas"]["SearchResults"];
export type ShareInfo = components["schemas"]["ShareInfo"];
export type SharePage = components["schemas"]["OffsetPage_MyShareInfo"];
export type SharePublicInfo = components["schemas"]["SharePublicInfo"];
export type ShareStatus = components["schemas"]["ShareStatus"];

// Admin, storage, and WebDAV
export type AuditLogEntry = components["schemas"]["AuditLogEntry"];
export type AuditLogPage = components["schemas"]["OffsetPage_AuditLogEntry"];
export type AdminOverview = components["schemas"]["AdminOverview"];
export type AdminOverviewDailyReport =
	components["schemas"]["AdminOverviewDailyReport"];
export type AdminOverviewStats = components["schemas"]["AdminOverviewStats"];
export type DriverType = components["schemas"]["DriverType"];
export type LockPage = components["schemas"]["OffsetPage_ResourceLock"];
export type RemovedCountResponse =
	components["schemas"]["RemovedCountResponse"];
export type StoragePolicy = components["schemas"]["StoragePolicy"];
export type StoragePolicyPage =
	components["schemas"]["OffsetPage_StoragePolicy"];
export type SystemConfig = components["schemas"]["SystemConfig"];
export type SystemConfigPage = components["schemas"]["OffsetPage_SystemConfig"];
export type WebdavAccount = components["schemas"]["WebdavAccount"];
export type WebdavAccountCreated =
	components["schemas"]["WebdavAccountCreated"];
export type WebdavAccountInfo = components["schemas"]["WebdavAccountInfo"];
export type WebdavAccountPage =
	components["schemas"]["OffsetPage_WebdavAccountInfo"];
export type WebdavSettingsInfo = components["schemas"]["WebdavSettingsInfo"];

// Upload and batch
export type BatchItemError = components["schemas"]["BatchItemError"];
export type BatchResult = components["schemas"]["BatchResult"];
export type ChunkUploadResponse = components["schemas"]["ChunkUploadResponse"];
export type CompletedPart = components["schemas"]["CompletedPartReq"];
export type InitUploadResponse = components["schemas"]["InitUploadResponse"];
export type UploadMode = components["schemas"]["UploadMode"];
export type UploadProgressResponse =
	components["schemas"]["UploadProgressResponse"];
export type UploadSessionStatus = components["schemas"]["UploadSessionStatus"];

// ErrorCode constants (generated type is a union of literal numbers, we need named constants)
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

// API response wrapper
export interface ApiResponse<T> {
	code: ErrorCode;
	msg: string;
	data: T | null;
}
