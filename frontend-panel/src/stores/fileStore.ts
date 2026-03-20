import { create } from 'zustand'
import { fileService } from '@/services/fileService'
import type { FileInfo, FolderInfo } from '@/types/api'

interface BreadcrumbItem {
  id: number | null
  name: string
}

interface FileState {
  currentFolderId: number | null
  folders: FolderInfo[]
  files: FileInfo[]
  breadcrumb: BreadcrumbItem[]
  loading: boolean
  navigateTo: (folderId: number | null, folderName?: string) => Promise<void>
  refresh: () => Promise<void>
  createFolder: (name: string) => Promise<void>
  deleteFile: (id: number) => Promise<void>
  deleteFolder: (id: number) => Promise<void>
}

export const useFileStore = create<FileState>((set, get) => ({
  currentFolderId: null,
  folders: [],
  files: [],
  breadcrumb: [{ id: null, name: 'Root' }],
  loading: false,

  navigateTo: async (folderId, folderName) => {
    set({ loading: true })
    try {
      const contents =
        folderId === null
          ? await fileService.listRoot()
          : await fileService.listFolder(folderId)

      // Update breadcrumb
      const { breadcrumb } = get()
      let newBreadcrumb: BreadcrumbItem[]

      if (folderId === null) {
        newBreadcrumb = [{ id: null, name: 'Root' }]
      } else {
        const existingIndex = breadcrumb.findIndex((b) => b.id === folderId)
        if (existingIndex >= 0) {
          // Going back up -- trim breadcrumb
          newBreadcrumb = breadcrumb.slice(0, existingIndex + 1)
        } else {
          // Going deeper
          newBreadcrumb = [
            ...breadcrumb,
            { id: folderId, name: folderName || `Folder ${folderId}` },
          ]
        }
      }

      set({
        currentFolderId: folderId,
        folders: contents.folders,
        files: contents.files,
        breadcrumb: newBreadcrumb,
        loading: false,
      })
    } catch (error) {
      set({ loading: false })
      throw error
    }
  },

  refresh: async () => {
    const { currentFolderId, navigateTo, breadcrumb } = get()
    const currentName = breadcrumb[breadcrumb.length - 1]?.name
    await navigateTo(currentFolderId, currentName)
  },

  createFolder: async (name) => {
    const { currentFolderId } = get()
    await fileService.createFolder(name, currentFolderId)
    await get().refresh()
  },

  deleteFile: async (id) => {
    await fileService.deleteFile(id)
    await get().refresh()
  },

  deleteFolder: async (id) => {
    await fileService.deleteFolder(id)
    await get().refresh()
  },
}))
