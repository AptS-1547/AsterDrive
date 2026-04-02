import { type DragEvent, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link, useLocation } from "react-router-dom";
import { FolderTree } from "@/components/folders/FolderTree";
import { Icon, type IconName } from "@/components/ui/icon";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
	USER_SIDEBAR_WIDTH_CLASS,
	USER_TOPBAR_OFFSET_CLASS,
} from "@/lib/constants";
import {
	hasInternalDragData,
	type InternalDragData,
	readInternalDragData,
} from "@/lib/dragDrop";
import { formatBytes } from "@/lib/format";
import { cn, sidebarNavItemClass } from "@/lib/utils";
import {
	isTeamWorkspace,
	workspaceRootPath,
	workspaceSharesPath,
	workspaceTrashPath,
	workspaceWebdavPath,
} from "@/lib/workspace";
import { useAuthStore } from "@/stores/authStore";
import { useTeamStore } from "@/stores/teamStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

interface SidebarProps {
	mobileOpen: boolean;
	onMobileClose: () => void;
	onTrashDrop?: (data: InternalDragData) => void | Promise<void>;
	onMoveToFolder?: (
		fileIds: number[],
		folderIds: number[],
		targetFolderId: number | null,
	) => Promise<void> | void;
}

export function Sidebar({
	mobileOpen,
	onMobileClose,
	onTrashDrop,
	onMoveToFolder,
}: SidebarProps) {
	const { t } = useTranslation();
	const location = useLocation();
	const user = useAuthStore((s) => s.user);
	const workspace = useWorkspaceStore((s) => s.workspace);
	const teams = useTeamStore((s) => s.teams);
	const loadingTeams = useTeamStore((s) => s.loading);
	const ensureTeamsLoaded = useTeamStore((s) => s.ensureLoaded);
	const [trashDragOver, setTrashDragOver] = useState(false);
	const activeTeam = isTeamWorkspace(workspace)
		? (teams.find((team) => team.id === workspace.teamId) ?? null)
		: null;

	useEffect(() => {
		void ensureTeamsLoaded(user?.id ?? null).catch(() => undefined);
	}, [ensureTeamsLoaded, user?.id]);

	const navLinks: { to: string; icon: IconName; label: string }[] =
		useMemo(() => {
			const links: { to: string; icon: IconName; label: string }[] = [
				{
					to: workspaceTrashPath(workspace),
					icon: "Trash",
					label: t("trash"),
				},
				{
					to: workspaceSharesPath(workspace),
					icon: "Link",
					label: t("share:my_shares_title"),
				},
			];

			if (!isTeamWorkspace(workspace)) {
				links.push({
					to: workspaceWebdavPath(),
					icon: "HardDrive",
					label: t("webdav"),
				});
			}

			return links;
		}, [t, workspace]);

	const storageUsed = activeTeam
		? activeTeam.storage_used
		: !isTeamWorkspace(workspace)
			? (user?.storage_used ?? 0)
			: 0;
	const storageQuota = activeTeam
		? activeTeam.storage_quota
		: !isTeamWorkspace(workspace)
			? (user?.storage_quota ?? 0)
			: 0;
	const trashPath = workspaceTrashPath(workspace);

	const handleTrashDragOver = (e: DragEvent<HTMLAnchorElement>) => {
		if (!onTrashDrop || !hasInternalDragData(e.dataTransfer)) return;
		e.preventDefault();
		e.dataTransfer.dropEffect = "move";
		setTrashDragOver(true);
	};

	const handleTrashDragLeave = (e: DragEvent<HTMLAnchorElement>) => {
		const nextTarget = e.relatedTarget;
		if (nextTarget instanceof Node && e.currentTarget.contains(nextTarget)) {
			return;
		}
		setTrashDragOver(false);
	};

	const handleTrashDrop = (e: DragEvent<HTMLAnchorElement>) => {
		setTrashDragOver(false);
		if (!onTrashDrop) return;
		e.preventDefault();
		const data = readInternalDragData(e.dataTransfer);
		if (!data) return;
		void onTrashDrop(data);
	};

	const sidebarContent = (
		<div className="flex flex-col h-full">
			<div className="p-2 space-y-1">
				<p className="px-2 text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
					{t("workspaces")}
				</p>
				<Link
					to={workspaceRootPath({ kind: "personal" })}
					onClick={onMobileClose}
					className={sidebarNavItemClass(
						!isTeamWorkspace(workspace),
						"justify-between",
					)}
				>
					<span className="inline-flex items-center gap-2">
						<Icon name="House" className="h-4 w-4 shrink-0" />
						{t("my_drive")}
					</span>
				</Link>
				{teams.map((team) => (
					<Link
						key={team.id}
						to={workspaceRootPath({ kind: "team", teamId: team.id })}
						onClick={onMobileClose}
						className={sidebarNavItemClass(
							isTeamWorkspace(workspace) && workspace.teamId === team.id,
							"justify-between",
						)}
					>
						<span className="inline-flex min-w-0 items-center gap-2">
							<Icon name="Cloud" className="h-4 w-4 shrink-0" />
							<span className="truncate">{team.name}</span>
						</span>
					</Link>
				))}
				{loadingTeams && teams.length === 0 ? (
					<div className="px-2 py-1 text-xs text-muted-foreground">
						{t("loading")}
					</div>
				) : null}
			</div>

			<Separator />

			{/* Folder tree */}
			<ScrollArea className="flex-1">
				<FolderTree onMoveToFolder={onMoveToFolder} />
			</ScrollArea>

			<Separator />

			{/* Navigation links */}
			<div className="p-2 space-y-1">
				{navLinks.map((link) => (
					<Link
						key={link.to}
						to={link.to}
						onClick={onMobileClose}
						onDragOver={link.to === trashPath ? handleTrashDragOver : undefined}
						onDragLeave={
							link.to === trashPath ? handleTrashDragLeave : undefined
						}
						onDrop={link.to === trashPath ? handleTrashDrop : undefined}
						className={sidebarNavItemClass(
							location.pathname === link.to,
							link.to === trashPath &&
								trashDragOver &&
								"bg-destructive/10 text-destructive ring-1 ring-destructive/30",
						)}
					>
						<Icon name={link.icon} className="h-4 w-4 shrink-0" />
						{link.label}
					</Link>
				))}
			</div>

			{/* Storage usage */}
			{user && (!isTeamWorkspace(workspace) || activeTeam) && (
				<>
					<Separator />
					<div className="p-3 space-y-1.5">
						<p className="text-xs font-medium text-muted-foreground">
							{activeTeam ? activeTeam.name : t("files:storage_space")}
						</p>
						<Progress
							value={
								storageQuota > 0
									? Math.min((storageUsed / storageQuota) * 100, 100)
									: 0
							}
							className="h-1.5"
						/>
						<p className="text-xs text-muted-foreground">
							{storageQuota > 0
								? t("files:storage_quota", {
										used: formatBytes(storageUsed),
										quota: formatBytes(storageQuota),
									})
								: t("files:storage_used", {
										used: formatBytes(storageUsed),
									})}
						</p>
					</div>
				</>
			)}
		</div>
	);

	return (
		<>
			{/* Mobile overlay backdrop */}
			{mobileOpen && (
				<button
					type="button"
					className={cn(
						"fixed inset-x-0 bg-black/50 z-40 md:hidden cursor-default",
						USER_TOPBAR_OFFSET_CLASS,
					)}
					onClick={onMobileClose}
					aria-label={t("close_sidebar")}
				/>
			)}

			{/* Sidebar - desktop inline, mobile overlay */}
			<aside
				className={cn(
					"border-r flex flex-col bg-background transition-all duration-200",
					// Desktop
					"hidden md:flex",
					USER_SIDEBAR_WIDTH_CLASS,
					// Mobile override when open
					mobileOpen &&
						cn(
							"fixed left-0 z-50 flex md:relative md:inset-y-auto md:z-auto",
							USER_SIDEBAR_WIDTH_CLASS,
							USER_TOPBAR_OFFSET_CLASS,
						),
				)}
			>
				{sidebarContent}
			</aside>
		</>
	);
}
