import { Folder } from 'lucide-react'
import { useFileStore } from '@/stores/fileStore'
import { cn } from '@/lib/utils'

export function FolderTree() {
  const folders = useFileStore((s) => s.folders)
  const currentFolderId = useFileStore((s) => s.currentFolderId)
  const navigateTo = useFileStore((s) => s.navigateTo)

  return (
    <div className="p-2 space-y-0.5">
      <button
        type="button"
        className={cn(
          'w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-sm hover:bg-accent transition-colors text-left',
          currentFolderId === null && 'bg-accent font-medium',
        )}
        onClick={() => navigateTo(null)}
      >
        <Folder className="h-4 w-4 text-muted-foreground" />
        Root
      </button>
      {folders.map((folder) => (
        <button
          type="button"
          key={folder.id}
          className={cn(
            'w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-sm hover:bg-accent transition-colors text-left pl-6',
            currentFolderId === folder.id && 'bg-accent font-medium',
          )}
          onClick={() => navigateTo(folder.id, folder.name)}
        >
          <Folder className="h-4 w-4 text-muted-foreground" />
          {folder.name}
        </button>
      ))}
    </div>
  )
}
