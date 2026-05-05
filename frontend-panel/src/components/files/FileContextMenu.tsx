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
	onOpen?: () => void;
	onChooseOpenMethod?: () => void;
	onDownload?: () => void;
	onArchiveExtract?: () => void;
	onArchiveCompress?: () => void;
	onArchiveDownload?: () => void;
	onPageShare: () => void;
	onDirectShare?: () => void;
	onCopy: () => void;
	onMove?: () => void;
	onToggleLock: () => void;
	onDelete: () => void;
	onRename?: () => void;
	onVersions?: () => void;
	onInfo: () => void;
	isLocked: boolean;
	isFolder: boolean;
	renderTrigger?: boolean;
}

export function FileContextMenu({
	children,
	onOpen,
	onChooseOpenMethod,
	onDownload,
	onArchiveExtract,
	onArchiveCompress,
	onArchiveDownload,
	onPageShare,
	onDirectShare,
	onCopy,
	onMove,
	onRename,
	onToggleLock,
	onDelete,
	onVersions,
	onInfo,
	isLocked,
	isFolder,
	renderTrigger = false,
}: FileContextMenuProps) {
	const { t } = useTranslation(["files", "share", "tasks"]);

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
				{onOpen && (
					<ContextMenuItem onClick={onOpen}>
						<Icon name="Eye" className="h-4 w-4 mr-2" />
						{t("open")}
					</ContextMenuItem>
				)}
				{!isFolder && onChooseOpenMethod && (
					<ContextMenuItem onClick={onChooseOpenMethod}>
						<Icon name="ListBullets" className="h-4 w-4 mr-2" />
						{t("open_with_action")}
					</ContextMenuItem>
				)}
				{onOpen || (!isFolder && onChooseOpenMethod) ? (
					<ContextMenuSeparator />
				) : null}
				{!isFolder && onDownload && (
					<ContextMenuItem onClick={onDownload}>
						<Icon name="Download" className="h-4 w-4 mr-2" />
						{t("download")}
					</ContextMenuItem>
				)}
				{!isFolder && onArchiveExtract && (
					<ContextMenuItem onClick={onArchiveExtract}>
						<Icon name="FolderOpen" className="h-4 w-4 mr-2" />
						{t("tasks:archive_extract_action")}
					</ContextMenuItem>
				)}
				{onArchiveCompress && (
					<ContextMenuItem onClick={onArchiveCompress}>
						<Icon name="FileZip" className="h-4 w-4 mr-2" />
						{t("tasks:archive_compress_action")}
					</ContextMenuItem>
				)}
				{isFolder && onArchiveDownload && (
					<ContextMenuItem onClick={onArchiveDownload}>
						<Icon name="Download" className="h-4 w-4 mr-2" />
						{t("tasks:archive_download_action")}
					</ContextMenuItem>
				)}
				<ContextMenuItem onClick={onPageShare}>
					<Icon name="Link" className="h-4 w-4 mr-2" />
					{t("share")}
				</ContextMenuItem>
				{!isFolder && onDirectShare && (
					<ContextMenuItem onClick={onDirectShare}>
						<Icon name="LinkSimple" className="h-4 w-4 mr-2" />
						{t("share:share_direct_link_action")}
					</ContextMenuItem>
				)}
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
				<ContextMenuItem onClick={onInfo}>
					<Icon name="Info" className="h-4 w-4 mr-2" />
					{t("info")}
				</ContextMenuItem>
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
					{t("core:delete")}
				</ContextMenuItem>
			</ContextMenuContent>
		</ContextMenu>
	);
}
