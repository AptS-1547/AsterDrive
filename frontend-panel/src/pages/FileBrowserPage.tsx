import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { AppLayout } from '@/components/layout/AppLayout'
import { PageHeader } from '@/components/layout/PageHeader'
import { FileList } from '@/components/files/FileList'
import { UploadArea } from '@/components/files/UploadArea'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { FolderPlus, LogOut } from 'lucide-react'
import { useFileStore } from '@/stores/fileStore'
import { useAuthStore } from '@/stores/authStore'
import { handleApiError } from '@/hooks/useApiError'
import { toast } from 'sonner'
import { ScrollArea } from '@/components/ui/scroll-area'

export default function FileBrowserPage() {
  const navigateTo = useFileStore((s) => s.navigateTo)
  const createFolder = useFileStore((s) => s.createFolder)
  const logout = useAuthStore((s) => s.logout)
  const [folderName, setFolderName] = useState('')
  const [dialogOpen, setDialogOpen] = useState(false)

  useEffect(() => {
    navigateTo(null).catch(handleApiError)
  }, [navigateTo])

  const handleCreateFolder = async (e: FormEvent) => {
    e.preventDefault()
    if (!folderName.trim()) return
    try {
      await createFolder(folderName.trim())
      toast.success('Folder created')
      setFolderName('')
      setDialogOpen(false)
    } catch (error) {
      handleApiError(error)
    }
  }

  return (
    <AppLayout>
      <PageHeader
        actions={
          <>
            <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
              <DialogTrigger render={<Button variant="outline" size="sm" />}>
                <FolderPlus className="h-4 w-4 mr-1" />
                New Folder
              </DialogTrigger>
              <DialogContent>
                <DialogHeader>
                  <DialogTitle>Create Folder</DialogTitle>
                </DialogHeader>
                <form onSubmit={handleCreateFolder} className="space-y-4">
                  <Input
                    placeholder="Folder name"
                    value={folderName}
                    onChange={(e) => setFolderName(e.target.value)}
                    autoFocus
                  />
                  <Button type="submit" className="w-full">
                    Create
                  </Button>
                </form>
              </DialogContent>
            </Dialog>
            <Button variant="ghost" size="sm" onClick={logout}>
              <LogOut className="h-4 w-4" />
            </Button>
          </>
        }
      />
      <UploadArea>
        <ScrollArea className="flex-1">
          <FileList />
        </ScrollArea>
      </UploadArea>
    </AppLayout>
  )
}
