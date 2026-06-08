import type { ReactNode, RefObject } from "react";
import { TeamManageShell } from "@/components/settings/team-manage-detail/TeamManageShell";
import type { TeamInfo, TeamMemberRole } from "@/types/api";
import type { TeamManageTab } from "./types";

interface TeamManageDialogViewProps {
	auditSection: ReactNode;
	canArchiveTeam: boolean;
	canManageTeam: boolean;
	contentRef: RefObject<HTMLDivElement | null>;
	currentTab: TeamManageTab;
	dangerSection: ReactNode;
	isPageLayout: boolean;
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
	canArchiveTeam,
	canManageTeam,
	contentRef,
	currentTab,
	dangerSection,
	isPageLayout,
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
	roleLabel,
	sidebarRef,
	team,
	usagePercentage,
	used,
	viewerRole,
	webdavSection,
}: TeamManageDialogViewProps) {
	return (
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
	);
}
