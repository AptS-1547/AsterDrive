import { FileTypeIcon } from "@/components/files/FileTypeIcon";
import { Card } from "@/components/ui/card";
import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Icon } from "@/components/ui/icon";
import { ItemCheckbox } from "@/components/ui/item-checkbox";
import { formatDateAbsolute } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { MyShareInfo } from "@/types/api";
import { MyShareStatusBadge } from "./MyShareStatusBadge";

interface MyShareCardLabels {
	active: string;
	copy: string;
	created: (date: string) => string;
	delete: string;
	deleted: string;
	edit: string;
	exhausted: string;
	expire: (date: string) => string;
	expired: string;
	never: string;
	open: string;
}

interface MyShareCardProps {
	labels: MyShareCardLabels;
	onCopy: (share: MyShareInfo) => void;
	onDelete: (share: MyShareInfo) => void;
	onEdit: (share: MyShareInfo) => void;
	onOpen: (share: MyShareInfo) => void;
	onToggleSelect: (shareId: number) => void;
	selected: boolean;
	share: MyShareInfo;
}

export function MyShareCard({
	labels,
	onCopy,
	onDelete,
	onEdit,
	onOpen,
	onToggleSelect,
	selected,
	share,
}: MyShareCardProps) {
	const isFolder = share.resource_type === "folder";

	return (
		<ContextMenu>
			<ContextMenuTrigger className="w-full">
				<Card
					className={cn(
						"cursor-pointer border bg-card/80 px-4 py-3 shadow-sm transition-all duration-150 hover:-translate-y-0.5 hover:bg-card hover:shadow-md dark:shadow-none dark:hover:shadow-none",
						selected && "border-primary bg-accent/35",
					)}
					onClick={() => onOpen(share)}
					role="button"
					tabIndex={0}
					onKeyDown={(event) => {
						if (event.key === "Enter") {
							onOpen(share);
						}
					}}
				>
					<div className="flex items-center gap-3">
						<ItemCheckbox
							checked={selected}
							onChange={() => onToggleSelect(share.id)}
							className="mt-0.5"
						/>
						<div className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-muted/45">
							{isFolder ? (
								<Icon name="Folder" className="size-5 text-amber-500" />
							) : (
								<FileTypeIcon
									mimeType=""
									fileName={share.resource_name}
									className="size-5"
								/>
							)}
						</div>
						<div className="min-w-0 flex-1">
							<span className="block truncate text-sm font-semibold">
								{share.resource_name}
							</span>
						</div>
						<div className="shrink-0">
							<div className="flex items-center gap-2">
								<MyShareStatusBadge
									status={share.status}
									activeLabel={labels.active}
									expiredLabel={labels.expired}
									exhaustedLabel={labels.exhausted}
									deletedLabel={labels.deleted}
								/>
							</div>
						</div>
					</div>

					<div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 pl-8 text-xs text-muted-foreground">
						<span>{labels.created(formatDateAbsolute(share.created_at))}</span>
						{share.expires_at ? (
							<span>{labels.expire(formatDateAbsolute(share.expires_at))}</span>
						) : (
							<span>{labels.never}</span>
						)}
						{share.has_password ? (
							<Icon name="Lock" className="size-3" />
						) : null}
					</div>
				</Card>
			</ContextMenuTrigger>
			<ContextMenuContent>
				<ContextMenuItem onClick={() => onEdit(share)}>
					<Icon name="PencilSimple" />
					{labels.edit}
				</ContextMenuItem>
				<ContextMenuItem onClick={() => onCopy(share)}>
					<Icon name="Copy" />
					{labels.copy}
				</ContextMenuItem>
				<ContextMenuItem onClick={() => onOpen(share)}>
					<Icon name="ArrowSquareOut" />
					{labels.open}
				</ContextMenuItem>
				<ContextMenuSeparator />
				<ContextMenuItem variant="destructive" onClick={() => onDelete(share)}>
					<Icon name="Trash" />
					{labels.delete}
				</ContextMenuItem>
			</ContextMenuContent>
		</ContextMenu>
	);
}
