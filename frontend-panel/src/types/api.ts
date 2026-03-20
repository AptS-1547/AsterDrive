// Mirror backend ErrorCode (serde_repr numbers)
// Using const object + type pattern instead of enum for erasableSyntaxOnly compatibility
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
} as const

export type ErrorCode = (typeof ErrorCode)[keyof typeof ErrorCode]

export interface ApiResponse<T> {
  code: ErrorCode
  msg: string
  data: T | null
}

export interface UserInfo {
  id: number
  username: string
  email: string
  role: string
  status: string
  storage_used: number
  created_at: string
  updated_at: string
}

export interface FileInfo {
  id: number
  name: string
  folder_id: number | null
  blob_id: number
  user_id: number
  mime_type: string
  created_at: string
  updated_at: string
}

export interface FolderInfo {
  id: number
  name: string
  parent_id: number | null
  user_id: number
  policy_id: number | null
  created_at: string
  updated_at: string
}

export interface FolderContents {
  folders: FolderInfo[]
  files: FileInfo[]
}
