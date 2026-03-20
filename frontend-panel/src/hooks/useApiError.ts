import { toast } from 'sonner'
import { ApiError } from '@/services/http'
import { ErrorCode } from '@/types/api'

const errorMessages: Partial<Record<ErrorCode, string>> = {
  [ErrorCode.AuthFailed]: 'Invalid username or password',
  [ErrorCode.TokenExpired]: 'Session expired, please login again',
  [ErrorCode.TokenInvalid]: 'Invalid token',
  [ErrorCode.Forbidden]: 'Permission denied',
  [ErrorCode.FileNotFound]: 'File not found',
  [ErrorCode.FileTooLarge]: 'File is too large',
  [ErrorCode.FileTypeNotAllowed]: 'File type not allowed',
  [ErrorCode.StorageQuotaExceeded]: 'Storage quota exceeded',
  [ErrorCode.FolderNotFound]: 'Folder not found',
}

export function handleApiError(error: unknown) {
  if (error instanceof ApiError) {
    const message = errorMessages[error.code] || error.message
    toast.error(message)
  } else if (error instanceof Error) {
    toast.error(error.message)
  } else {
    toast.error('An unexpected error occurred')
  }
}
