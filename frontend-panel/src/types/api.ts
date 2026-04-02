// Re-export generated types for convenience
import type { components } from "@/services/api.generated";

// Core responses
export type ErrorCode = components["schemas"]["ErrorCode"];
export type HealthResponse = components["schemas"]["HealthResponse"];
export type MemoryStatsResponse = components["schemas"]["MemoryStatsResponse"];

// Auth and user
export type AvatarInfo = components["schemas"]["AvatarInfo"];
export type AvatarSource = components["schemas"]["AvatarSource"];
export type AuthTokenResp = components["schemas"]["AuthTokenResp"];
export type CheckResp = components["schemas"]["CheckResp"];
export type ChangePasswordRequest = components["schemas"]["ChangePasswordReq"];
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
export type CreatePolicyGroupRequest =
	components["schemas"]["CreatePolicyGroupReq"];
export type CreatePolicyRequest = components["schemas"]["CreatePolicyReq"];
export type DriverType = components["schemas"]["DriverType"];
export type LockPage = components["schemas"]["OffsetPage_ResourceLock"];
export type MigratePolicyGroupUsersRequest =
	components["schemas"]["MigratePolicyGroupUsersReq"];
export type PolicyGroupItemRequest =
	components["schemas"]["PolicyGroupItemReq"];
export type UpdatePolicyGroupRequest =
	components["schemas"]["PatchPolicyGroupReq"];
export type UpdatePolicyRequest = components["schemas"]["PatchPolicyReq"];
export type UpdateUserRequest = components["schemas"]["PatchUserReq"];
export type RemovedCountResponse =
	components["schemas"]["RemovedCountResponse"];
export type ResetUserPasswordRequest =
	components["schemas"]["ResetUserPasswordReq"];
export type StoragePolicy = components["schemas"]["StoragePolicy"];
export type StoragePolicyPage =
	components["schemas"]["OffsetPage_StoragePolicy"];
export type StoragePolicyGroupItem =
	components["schemas"]["StoragePolicyGroupItemInfo"];
export type StoragePolicyGroup =
	components["schemas"]["StoragePolicyGroupInfo"];
export type PolicyGroupUserMigrationResult =
	components["schemas"]["PolicyGroupUserMigrationResult"];
export type StoragePolicyGroupPage =
	components["schemas"]["OffsetPage_StoragePolicyGroupInfo"];
export type SystemConfig = components["schemas"]["SystemConfig"];
export type SystemConfigPage = components["schemas"]["OffsetPage_SystemConfig"];
export type WebdavAccount = components["schemas"]["WebdavAccount"];
export type WebdavAccountCreated =
	components["schemas"]["WebdavAccountCreated"];
export type WebdavAccountInfo = components["schemas"]["WebdavAccountInfo"];
export type WebdavAccountPage =
	components["schemas"]["OffsetPage_WebdavAccountInfo"];
export type WebdavSettingsInfo = components["schemas"]["WebdavSettingsInfo"];

// Teams
export type TeamInfo = components["schemas"]["TeamInfo"];
export type TeamMemberInfo = components["schemas"]["TeamMemberInfo"];
export type TeamMemberRole = components["schemas"]["TeamMemberRole"];

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
