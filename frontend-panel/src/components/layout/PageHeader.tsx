import type { ReactNode } from 'react'
import { Fragment } from 'react'
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb'
import { useFileStore } from '@/stores/fileStore'

interface PageHeaderProps {
  actions?: ReactNode
}

export function PageHeader({ actions }: PageHeaderProps) {
  const breadcrumb = useFileStore((s) => s.breadcrumb)
  const navigateTo = useFileStore((s) => s.navigateTo)

  return (
    <div className="flex items-center justify-between px-4 py-3 border-b">
      <Breadcrumb>
        <BreadcrumbList>
          {breadcrumb.map((item, i) => (
            <Fragment key={item.id ?? 'root'}>
              {i > 0 && <BreadcrumbSeparator />}
              <BreadcrumbItem>
                {i < breadcrumb.length - 1 ? (
                  <BreadcrumbLink
                    className="cursor-pointer"
                    onClick={() => navigateTo(item.id, item.name)}
                  >
                    {item.name}
                  </BreadcrumbLink>
                ) : (
                  <span className="font-medium">{item.name}</span>
                )}
              </BreadcrumbItem>
            </Fragment>
          ))}
        </BreadcrumbList>
      </Breadcrumb>
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </div>
  )
}
