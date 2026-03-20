import { api } from './http'
import { endpoints } from './endpoints'
import type { FileInfo, FolderInfo, FolderContents } from '@/types/api'

export const fileService = {
  listRoot: () => api.get<FolderContents>(endpoints.folders.root),

  listFolder: (id: number) => api.get<FolderContents>(endpoints.folders.get(id)),

  createFolder: (name: string, parentId?: number | null) =>
    api.post<FolderInfo>(endpoints.folders.create, {
      name,
      parent_id: parentId ?? null,
    }),

  deleteFolder: (id: number) => api.delete<void>(endpoints.folders.delete(id)),

  renameFolder: (id: number, name: string) =>
    api.patch<FolderInfo>(endpoints.folders.update(id), { name }),

  getFile: (id: number) => api.get<FileInfo>(endpoints.files.get(id)),

  deleteFile: (id: number) => api.delete<void>(endpoints.files.delete(id)),

  renameFile: (id: number, name: string) =>
    api.patch<FileInfo>(endpoints.files.update(id), { name }),

  downloadUrl: (id: number) =>
    `${api.client.defaults.baseURL}${endpoints.files.download(id)}`,
}
