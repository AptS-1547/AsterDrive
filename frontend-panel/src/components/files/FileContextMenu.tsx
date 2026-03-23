import type { ReactNode } from "react";
import { isValidElement } from "react";
import { useTranslation } from "react-i18next";
import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Icon } from "@/components/ui/icon";

interface FileContextMenuProps {
	children: ReactNode;
	onDownload?: () => void;
	onShare: () => void;
	onCopy: () => void;
	onMove?: () => void;
	onToggleLock: () => void;
	onDelete: () => void;
	onRename?: () => void;
	onVersions?: () => void;
	isLocked: boolean;
	isFolder: boolean;
	renderTrigger?: boolean;
}

export function FileContextMenu({
	children,
	onDownload,
	onShare,
	onCopy,
	onMove,
	onRename,
	onToggleLock,
	onDelete,
	onVersions,
	isLocked,
	isFolder,
	renderTrigger = false,
}: FileContextMenuProps) {
	const { t } = useTranslation("files");

	const trigger =
		renderTrigger && isValidElement(children) ? (
			<ContextMenuTrigger render={children} />
		) : (
			<ContextMenuTrigger className="w-full">{children}</ContextMenuTrigger>
		);

	return (
		<ContextMenu>
			{trigger}
			<ContextMenuContent>
				{!isFolder && onDownload && (
					<ContextMenuItem onClick={onDownload}>
						<Icon name="Download" className="h-4 w-4 mr-2" />
						{t("download")}
					</ContextMenuItem>
				)}
				<ContextMenuItem onClick={onShare}>
					<Icon name="Link" className="h-4 w-4 mr-2" />
					{t("share")}
				</ContextMenuItem>
				<ContextMenuItem onClick={onCopy}>
					<Icon name="Copy" className="h-4 w-4 mr-2" />
					{t("copy")}
				</ContextMenuItem>
				{onMove && (
					<ContextMenuItem onClick={onMove}>
						<Icon name="ArrowsOutCardinal" className="h-4 w-4 mr-2" />
						{t("move")}
					</ContextMenuItem>
				)}
				{onRename && (
					<ContextMenuItem onClick={onRename}>
						<Icon name="PencilSimple" className="h-4 w-4 mr-2" />
						{t("rename")}
					</ContextMenuItem>
				)}
				{!isFolder && onVersions && (
					<ContextMenuItem onClick={onVersions}>
						<Icon name="Clock" className="h-4 w-4 mr-2" />
						{t("versions")}
					</ContextMenuItem>
				)}
				<ContextMenuSeparator />
				<ContextMenuItem onClick={onToggleLock}>
					{isLocked ? (
						<>
							<Icon name="LockOpen" className="h-4 w-4 mr-2" />
							{t("unlock")}
						</>
					) : (
						<>
							<Icon name="Lock" className="h-4 w-4 mr-2" />
							{t("lock")}
						</>
					)}
				</ContextMenuItem>
				<ContextMenuItem
					onClick={onDelete}
					disabled={isLocked}
					className="text-destructive"
				>
					<Icon name="Trash" className="h-4 w-4 mr-2" />
					{t("common:delete")}
				</ContextMenuItem>
			</ContextMenuContent>
		</ContextMenu>
	);
}
