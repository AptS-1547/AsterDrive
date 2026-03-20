import { useEffect, useRef, useState } from 'react'
import type { DragEvent, ReactNode } from 'react'
import { Uppy } from '@uppy/core'
import XHRUpload from '@uppy/xhr-upload'
import { Upload } from 'lucide-react'
import { config } from '@/config/app'
import { useFileStore } from '@/stores/fileStore'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'

interface UploadAreaProps {
  children: ReactNode
}

/**
 * 包裹子组件的拖拽上传区域。
 * 文件拖入时显示半透明 overlay，松手自动上传到当前文件夹。
 */
export function UploadArea({ children }: UploadAreaProps) {
  const refresh = useFileStore((s) => s.refresh)
  const currentFolderId = useFileStore((s) => s.currentFolderId)
  const currentFolderIdRef = useRef(currentFolderId)
  const [isDragging, setIsDragging] = useState(false)
  const dragCounter = useRef(0)

  // 保持 ref 同步，Uppy 闭包里用 ref 取最新值
  useEffect(() => {
    currentFolderIdRef.current = currentFolderId
  }, [currentFolderId])

  const [uppy] = useState(() => {
    const instance = new Uppy({
      restrictions: { maxNumberOfFiles: 10 },
      autoProceed: true,
    })
    instance.use(XHRUpload, {
      endpoint: `${config.apiBaseUrl}/files/upload`,
      fieldName: 'file',
      withCredentials: true,
    })
    instance.on('complete', (result) => {
      const count = result.successful?.length ?? 0
      if (count > 0) {
        toast.success(`Uploaded ${count} file(s)`)
        refresh()
      }
      // 清除已完成的文件，允许再次上传同名文件
      instance.cancelAll()
    })
    instance.on('error', (error) => {
      toast.error(`Upload failed: ${error.message}`)
    })
    return instance
  })

  // 动态更新 endpoint 以包含当前 folder_id
  useEffect(() => {
    const folderId = currentFolderIdRef.current
    const base = `${config.apiBaseUrl}/files/upload`
    const endpoint = folderId !== null ? `${base}?folder_id=${folderId}` : base
    const xhrPlugin = uppy.getPlugin('XHRUpload')
    if (xhrPlugin) {
      xhrPlugin.setOptions({ endpoint })
    }
  }, [currentFolderId, uppy])

  useEffect(() => {
    return () => uppy.destroy()
  }, [uppy])

  const addFiles = (files: FileList | null) => {
    if (!files || files.length === 0) return
    for (const file of files) {
      try {
        uppy.addFile({
          name: file.name,
          type: file.type,
          data: file,
        })
      } catch (err) {
        if (err instanceof Error && !err.message.includes('already been added')) {
          toast.error(err.message)
        }
      }
    }
  }

  const handleDragEnter = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault()
    dragCounter.current += 1
    if (e.dataTransfer.types.includes('Files')) {
      setIsDragging(true)
    }
  }

  const handleDragLeave = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault()
    dragCounter.current -= 1
    if (dragCounter.current === 0) {
      setIsDragging(false)
    }
  }

  const handleDragOver = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault()
  }

  const handleDrop = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault()
    dragCounter.current = 0
    setIsDragging(false)
    addFiles(e.dataTransfer.files)
  }

  return (
    <div
      className="relative flex-1 flex flex-col overflow-hidden"
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      {children}

      {/* 拖拽 overlay */}
      {isDragging && (
        <div className={cn(
          'absolute inset-0 z-50 flex flex-col items-center justify-center',
          'bg-background/80 backdrop-blur-sm border-2 border-dashed border-primary rounded-lg',
        )}>
          <Upload className="h-10 w-10 text-primary mb-3" />
          <p className="text-lg font-medium text-primary">Drop files to upload</p>
          <p className="text-sm text-muted-foreground mt-1">
            Files will be uploaded to the current folder
          </p>
        </div>
      )}
    </div>
  )
}
