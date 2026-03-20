import { FileIcon, Folder, Download, Trash2 } from 'lucide-react'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Button } from '@/components/ui/button'
import { useFileStore } from '@/stores/fileStore'
import { fileService } from '@/services/fileService'
import { handleApiError } from '@/hooks/useApiError'
import { toast } from 'sonner'

function formatDate(dateStr: string): string {
  const date = new Date(dateStr)
  const now = new Date()
  const diff = now.getTime() - date.getTime()
  const minutes = Math.floor(diff / 60000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  if (days < 30) return `${days}d ago`
  return date.toLocaleDateString()
}

export function FileList() {
  const folders = useFileStore((s) => s.folders)
  const files = useFileStore((s) => s.files)
  const navigateTo = useFileStore((s) => s.navigateTo)
  const deleteFile = useFileStore((s) => s.deleteFile)
  const deleteFolder = useFileStore((s) => s.deleteFolder)
  const loading = useFileStore((s) => s.loading)

  const handleDownload = (fileId: number, fileName: string) => {
    const url = fileService.downloadUrl(fileId)
    fetch(url, { credentials: 'include' })
      .then((res) => res.blob())
      .then((blob) => {
        const objectUrl = URL.createObjectURL(blob)
        const a = document.createElement('a')
        a.href = objectUrl
        a.download = fileName
        a.click()
        URL.revokeObjectURL(objectUrl)
      })
      .catch((err) => handleApiError(err))
  }

  const handleDeleteFile = async (id: number) => {
    try {
      await deleteFile(id)
      toast.success('File deleted')
    } catch (error) {
      handleApiError(error)
    }
  }

  const handleDeleteFolder = async (id: number) => {
    try {
      await deleteFolder(id)
      toast.success('Folder deleted')
    } catch (error) {
      handleApiError(error)
    }
  }

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        Loading...
      </div>
    )
  }

  if (folders.length === 0 && files.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        This folder is empty
      </div>
    )
  }

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead className="w-[50%]">Name</TableHead>
          <TableHead>Modified</TableHead>
          <TableHead className="w-[100px]">Actions</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {folders.map((folder) => (
          <TableRow
            key={`folder-${folder.id}`}
            className="cursor-pointer"
            onClick={() => navigateTo(folder.id, folder.name)}
          >
            <TableCell className="flex items-center gap-2">
              <Folder className="h-4 w-4 text-blue-500" />
              {folder.name}
            </TableCell>
            <TableCell className="text-muted-foreground">
              {formatDate(folder.updated_at)}
            </TableCell>
            <TableCell>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8"
                onClick={(e) => {
                  e.stopPropagation()
                  handleDeleteFolder(folder.id)
                }}
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            </TableCell>
          </TableRow>
        ))}
        {files.map((file) => (
          <TableRow key={`file-${file.id}`}>
            <TableCell className="flex items-center gap-2">
              <FileIcon className="h-4 w-4 text-muted-foreground" />
              {file.name}
            </TableCell>
            <TableCell className="text-muted-foreground">
              {formatDate(file.updated_at)}
            </TableCell>
            <TableCell>
              <div className="flex gap-1">
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8"
                  onClick={() => handleDownload(file.id, file.name)}
                >
                  <Download className="h-4 w-4" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8"
                  onClick={() => handleDeleteFile(file.id)}
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              </div>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
