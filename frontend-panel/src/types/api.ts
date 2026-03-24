// Re-export generated types for convenience
import type { components } from "@/services/api.generated";

// Schema types
export type UserInfo = components["schemas"]["UserInfo"];
export type FileInfo = components["schemas"]["FileInfo"];
export type FolderInfo = components["schemas"]["FolderInfo"];
export type StoragePolicy = components["schemas"]["StoragePolicy"];
export type SystemConfig = components["schemas"]["SystemConfig"];
export type FolderContents = components["schemas"]["FolderContentsResponse"];
export type ErrorCode = components["schemas"]["ErrorCode"];
export type UserRole = components["schemas"]["UserRole"];
export type UserStatus = components["schemas"]["UserStatus"];
export type DriverType = components["schemas"]["DriverType"];
export type ShareInfo = components["schemas"]["ShareInfo"];
export type SharePublicInfo = components["schemas"]["SharePublicInfo"];
export type UserStoragePolicy = components["schemas"]["UserStoragePolicy"];
export type WebdavAccount = components["schemas"]["WebdavAccount"];
export type WebdavAccountCreated =
	components["schemas"]["WebdavAccountCreated"];
export type WebdavAccountInfo = components["schemas"]["WebdavAccountInfo"];

export interface TrashFileItem {
	id: number;
	name: string;
	size: number;
	mime_type: string;
	created_at: string;
	updated_at: string;
	deleted_at: string;
	is_locked: boolean;
	original_path: string;
}

export interface TrashFolderItem {
	id: number;
	name: string;
	created_at: string;
	updated_at: string;
	deleted_at: string;
	is_locked: boolean;
	original_path: string;
}

export type TrashItem =
	| (TrashFileItem & { entity_type: "file" })
	| (TrashFolderItem & { entity_type: "folder" });

export interface TrashContents {
	files: TrashFileItem[];
	folders: TrashFolderItem[];
}

// Auth
export type CheckResp = components["schemas"]["CheckResp"];

// Search
export type FileSearchItem = components["schemas"]["FileSearchItem"];
export type SearchParams = components["schemas"]["SearchParams"];
export type SearchResults = components["schemas"]["SearchResults"];

// Batch
export type BatchResult = components["schemas"]["BatchResult"];
export type BatchItemError = components["schemas"]["BatchItemError"];

// Audit
export type AuditLogEntry = components["schemas"]["AuditLogEntry"];
export type AuditLogPage = components["schemas"]["AuditLogPage"];

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
	ThumbnailFailed: 3008,
} as const satisfies Record<string, ErrorCode>;

// API response wrapper
export interface ApiResponse<T> {
	code: ErrorCode;
	msg: string;
	data: T | null;
}
