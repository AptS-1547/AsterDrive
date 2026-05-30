import type { ReactNode, RefObject } from "react";
import { TeamManageConfirmDialogs } from "@/components/settings/team-manage-detail/TeamManageConfirmDialogs";
import { TeamManageShell } from "@/components/settings/team-manage-detail/TeamManageShell";
import type { TeamInfo, TeamMemberInfo, TeamMemberRole } from "@/types/api";
import type { TeamManageTab } from "./types";

interface TeamManageDialogViewProps {
	auditSection: ReactNode;
	archiveConfirmLabel: string;
	archiveDescription: string;
	archiveDialogProps: {
		onOpenChange: (open: boolean) => void;
		open: boolean;
		onConfirm: () => void;
	};
	canArchiveTeam: boolean;
	canManageTeam: boolean;
	contentRef: RefObject<HTMLDivElement | null>;
	currentTab: TeamManageTab;
	currentUserId: number | null;
	dangerSection: ReactNode;
	isPageLayout: boolean;
	leaveLabel: string;
	managerCount: number;
	membersSection: ReactNode;
	onContentScroll: () => void;
	onOpenChange: (open: boolean) => void;
	onOpenWorkspace: () => void;
	onPageBack: () => void;
	onSidebarScroll: () => void;
	onTabChange: (value: string) => void;
	open: boolean;
	overviewSection: ReactNode;
	ownerCount: number;
	panelAnimationClass: string;
	quota: number;
	removeDescription: string;
	removeDialogProps: {
		onOpenChange: (open: boolean) => void;
		open: boolean;
		onConfirm: () => void;
	};
	removeLabel: string;
	removeMember: TeamMemberInfo | null;
	roleLabel: (role: TeamMemberRole) => string;
	sidebarRef: RefObject<HTMLElement | null>;
	team: TeamInfo | null;
	usagePercentage: number;
	used: number;
	viewerRole: TeamMemberRole | null;
	webdavSection: ReactNode;
}

export function TeamManageDialogView({
	auditSection,
	archiveConfirmLabel,
	archiveDescription,
	archiveDialogProps,
	canArchiveTeam,
	canManageTeam,
	contentRef,
	currentTab,
	currentUserId,
	dangerSection,
	isPageLayout,
	leaveLabel,
	managerCount,
	membersSection,
	onContentScroll,
	onOpenChange,
	onOpenWorkspace,
	onPageBack,
	onSidebarScroll,
	onTabChange,
	open,
	overviewSection,
	ownerCount,
	panelAnimationClass,
	quota,
	removeDescription,
	removeDialogProps,
	removeLabel,
	removeMember,
	roleLabel,
	sidebarRef,
	team,
	usagePercentage,
	used,
	viewerRole,
	webdavSection,
}: TeamManageDialogViewProps) {
	return (
		<>
			<TeamManageShell
				auditSection={auditSection}
				canArchiveTeam={canArchiveTeam}
				canManageTeam={canManageTeam}
				contentRef={contentRef}
				currentTab={currentTab}
				dangerSection={dangerSection}
				isPageLayout={isPageLayout}
				managerCount={managerCount}
				membersSection={membersSection}
				onContentScroll={onContentScroll}
				onOpenChange={onOpenChange}
				onOpenWorkspace={onOpenWorkspace}
				onPageBack={onPageBack}
				onSidebarScroll={onSidebarScroll}
				onTabChange={onTabChange}
				open={open}
				overviewSection={overviewSection}
				ownerCount={ownerCount}
				panelAnimationClass={panelAnimationClass}
				quota={quota}
				roleLabel={roleLabel}
				sidebarRef={sidebarRef}
				team={team}
				usagePercentage={usagePercentage}
				used={used}
				viewerRole={viewerRole}
				webdavSection={webdavSection}
			/>

			<TeamManageConfirmDialogs
				archiveConfirmLabel={archiveConfirmLabel}
				archiveDescription={archiveDescription}
				archiveDialogProps={archiveDialogProps}
				currentUserId={currentUserId}
				leaveLabel={leaveLabel}
				removeDescription={removeDescription}
				removeDialogProps={removeDialogProps}
				removeLabel={removeLabel}
				removeMember={removeMember}
			/>
		</>
	);
}
