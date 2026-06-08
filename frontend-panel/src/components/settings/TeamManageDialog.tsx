import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { TeamManageDialogView } from "@/components/settings/team-manage-detail/TeamManageDialogView";
import type { TeamManageTab } from "@/components/settings/team-manage-detail/types";
import { useTeamManageActions } from "@/components/settings/team-manage-detail/useTeamManageActions";
import { useTeamManageData } from "@/components/settings/team-manage-detail/useTeamManageData";
import { useTeamManageLocalState } from "@/components/settings/team-manage-detail/useTeamManageLocalState";
import { useTeamManageScrollRestoration } from "@/components/settings/team-manage-detail/useTeamManageScrollRestoration";
import { buildTeamManageSections } from "@/components/settings/team-manage-detail/useTeamManageSections";
import { useTeamManageTabs } from "@/components/settings/team-manage-detail/useTeamManageTabs";
import { useTeamManageViewModel } from "@/components/settings/team-manage-detail/useTeamManageViewModel";
import { handleApiError } from "@/hooks/useApiError";
import { normalizeWebdavPrefix } from "@/lib/webdav";
import { webdavAccountService } from "@/services/webdavAccountService";
import type { TeamInfo, TeamMemberRole } from "@/types/api";

export type { TeamManageTab } from "@/components/settings/team-manage-detail/types";

interface TeamManageDialogProps {
	currentUserId: number | null;
	layout?: "dialog" | "page";
	onArchivedReload: () => Promise<void>;
	onOpenChange: (open: boolean) => void;
	onPageTabChange?: (
		tab: TeamManageTab,
		options?: { replace?: boolean },
	) => void;
	onTeamsReload: () => Promise<void>;
	open: boolean;
	pageTab?: TeamManageTab;
	teamId: number | null;
	teamSummary: TeamInfo | null;
}

export function TeamManageDialog({
	currentUserId,
	layout = "dialog",
	onArchivedReload,
	onOpenChange,
	onPageTabChange,
	onTeamsReload,
	open,
	pageTab,
	teamId,
	teamSummary,
}: TeamManageDialogProps) {
	const { t } = useTranslation(["core", "settings"]);
	const navigate = useNavigate();
	const isPageLayout = layout === "page";
	const activeTeamId = open ? teamId : null;
	const localState = useTeamManageLocalState(activeTeamId);
	const {
		archiveConfirmValue,
		auditOffset,
		memberIdentifier,
		memberOffset,
		memberQuery,
		memberRole,
		memberRoleFilter,
		memberStatusFilter,
		setArchiveConfirmValue,
		setAuditOffset,
		setMemberIdentifier,
		setMemberOffset,
		setMemberQuery,
		setMemberRole,
		setMemberRoleFilter,
		setMemberStatusFilter,
		setTeamDraft,
		setWebdavPrefix,
		teamDraft,
		webdavPrefix,
	} = localState;
	const roleLabel = (role: TeamMemberRole) =>
		t(`settings:settings_team_role_${role}`);
	const viewModel = useTeamManageViewModel({
		activeTeamId,
		auditOffset,
		auditTotal: 0,
		canAssignOwner: false,
		displayTeam: null,
		memberOffset,
		memberQuery,
		memberRoleFilter,
		memberStatusFilter,
		memberTotal: 0,
		roleLabel,
		t,
		teamDraft,
	});
	const {
		auditEntries,
		auditLoading,
		auditTotal,
		canArchiveTeam,
		canAssignOwner,
		canManageTeam,
		detailLoading,
		detailRequestStarted,
		displayTeam,
		loadAuditEntries,
		loadMembers,
		loadTeamDetail,
		managerCount,
		memberLoading,
		memberTotal,
		members,
		ownerCount,
		teamDetail,
		viewerRole,
	} = useTeamManageData({
		auditOffset,
		memberFilters: viewModel.memberFilters,
		memberOffset,
		open,
		teamId,
		teamSummary,
	});
	const {
		auditCurrentPage,
		auditTotalPages,
		hasMemberFilters,
		memberCurrentPage,
		memberTotalPages,
		nextAuditPageDisabled,
		nextMemberPageDisabled,
		prevAuditPageDisabled,
		prevMemberPageDisabled,
		quota,
		roleFilterOptions,
		roleOptions,
		safeMemberOffset,
		statusFilterOptions,
		teamBaseDescription,
		teamBaseName,
		teamDescription,
		teamName,
		usagePercentage,
		used,
	} = useTeamManageViewModel({
		activeTeamId,
		auditOffset,
		auditTotal,
		canAssignOwner,
		displayTeam,
		memberOffset,
		memberQuery,
		memberRoleFilter,
		memberStatusFilter,
		memberTotal,
		roleLabel,
		t,
		teamDraft,
	});
	const { contentRef, handleContentScroll, handleSidebarScroll, sidebarRef } =
		useTeamManageScrollRestoration({
			isPageLayout,
			pageTab,
			teamId,
		});
	const { currentTab, handleTabChange, panelAnimationClass } =
		useTeamManageTabs({
			canArchiveTeam,
			canManageTeam,
			detailLoading,
			detailRequestStarted,
			isPageLayout,
			onPageTabChange,
			pageTab,
		});
	const setTeamName = (name: string) => {
		setTeamDraft({
			baseDescription: teamBaseDescription,
			baseName: teamBaseName,
			description: teamDescription,
			name,
			teamId: activeTeamId,
		});
	};
	const setTeamDescription = (description: string) => {
		setTeamDraft({
			baseDescription: teamBaseDescription,
			baseName: teamBaseName,
			description,
			name: teamName,
			teamId: activeTeamId,
		});
	};

	useEffect(() => {
		if (!open) {
			return;
		}

		let cancelled = false;
		void webdavAccountService
			.settings()
			.then((settings) => {
				if (!cancelled) {
					setWebdavPrefix(normalizeWebdavPrefix(settings.prefix));
				}
			})
			.catch(handleApiError);

		return () => {
			cancelled = true;
		};
	}, [open, setWebdavPrefix]);

	const {
		handleAddMember,
		handleArchiveTeam,
		handleRemoveMember,
		handleUpdateMemberRole,
		handleUpdateTeam,
		mutating,
	} = useTeamManageActions({
		canArchiveTeam,
		canManageTeam,
		currentUserId,
		loadAuditEntries,
		loadMembers,
		loadTeamDetail,
		onArchivedReload,
		onOpenChange,
		onTeamsReload,
		safeMemberOffset,
		setMemberIdentifier,
		setMemberOffset,
		setMemberRole,
		teamDetail,
		teamId,
	});

	if (teamId == null) {
		return null;
	}

	const {
		auditSection,
		dangerSection,
		membersSection,
		overviewSection,
		webdavSection,
	} = buildTeamManageSections({
		archiveConfirmValue,
		auditCurrentPage,
		auditEntries,
		auditLoading,
		auditOffset,
		auditTotal,
		auditTotalPages,
		canArchiveTeam,
		canAssignOwner,
		canManageTeam,
		currentUserId,
		detailLoading,
		displayTeam,
		handleArchiveTeam,
		handleRemoveMember,
		handleUpdateMemberRole,
		hasMemberFilters,
		managerCount,
		memberCurrentPage,
		memberIdentifier,
		memberLoading,
		memberOffset: safeMemberOffset,
		memberQuery,
		memberRole,
		memberRoleFilter,
		memberStatusFilter,
		memberTotal,
		memberTotalPages,
		members,
		mutating,
		nextAuditPageDisabled,
		nextMemberPageDisabled,
		onAddMember: (event) => {
			event.preventDefault();
			void handleAddMember(memberIdentifier, memberRole);
		},
		onUpdateTeam: (event) => {
			event.preventDefault();
			void handleUpdateTeam(teamName, teamDescription);
		},
		ownerCount,
		prevAuditPageDisabled,
		prevMemberPageDisabled,
		roleFilterOptions,
		roleLabel,
		roleOptions,
		setArchiveConfirmValue,
		setAuditOffset,
		setMemberIdentifier,
		setMemberOffset,
		setMemberQuery,
		setMemberRole,
		setMemberRoleFilter,
		setMemberStatusFilter,
		setTeamDescription,
		setTeamName,
		statusFilterOptions,
		teamDescription,
		teamId,
		teamName,
		viewerRole,
		webdavPrefix,
	});

	return (
		<TeamManageDialogView
			auditSection={auditSection}
			canArchiveTeam={canArchiveTeam}
			canManageTeam={canManageTeam}
			contentRef={contentRef}
			currentTab={currentTab}
			dangerSection={dangerSection}
			isPageLayout={isPageLayout}
			managerCount={managerCount}
			membersSection={membersSection}
			onContentScroll={handleContentScroll}
			onOpenChange={onOpenChange}
			onOpenWorkspace={() =>
				navigate(`/teams/${teamId}`, { viewTransition: false })
			}
			onPageBack={() => onOpenChange(false)}
			onSidebarScroll={handleSidebarScroll}
			onTabChange={handleTabChange}
			open={open}
			overviewSection={overviewSection}
			ownerCount={ownerCount}
			panelAnimationClass={panelAnimationClass}
			quota={quota}
			roleLabel={roleLabel}
			sidebarRef={sidebarRef}
			team={displayTeam}
			usagePercentage={usagePercentage}
			used={used}
			viewerRole={viewerRole}
			webdavSection={webdavSection}
		/>
	);
}
