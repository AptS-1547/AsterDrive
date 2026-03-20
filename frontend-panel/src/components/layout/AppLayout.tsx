import type { ReactNode } from 'react'
import { FolderTree } from '@/components/folders/FolderTree'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'

interface AppLayoutProps {
  children: ReactNode
}

export function AppLayout({ children }: AppLayoutProps) {
  return (
    <div className="h-screen flex flex-col">
      <div className="flex flex-1 overflow-hidden">
        {/* Left sidebar - Folder tree */}
        <aside className="w-60 border-r flex flex-col">
          <div className="p-3 font-semibold text-sm text-muted-foreground uppercase tracking-wider">
            Folders
          </div>
          <Separator />
          <ScrollArea className="flex-1">
            <FolderTree />
          </ScrollArea>
        </aside>

        {/* Right content */}
        <main className="flex-1 flex flex-col overflow-hidden">{children}</main>
      </div>
    </div>
  )
}
