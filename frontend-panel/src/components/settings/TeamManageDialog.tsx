import { type FormEvent, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import {
	TeamManageAuditSection,
	TeamManageDangerSection,
	TeamManageMembersSection,
	TeamManageOverviewSection,
} from "@/components/settings/team-manage-detail/TeamManageSections";
import { TeamManageShell } from "@/components/settings/team-manage-detail/TeamManageShell";
import {
	TEAM_MANAGE_AUDIT_PAGE_SIZE,
	TEAM_MANAGE_MEMBER_PAGE_SIZE,
} from "@/components/settings/team-manage-detail/teamManageDialogState";
import type { TeamManageTab } from "@/components/settings/team-manage-detail/types";
import { useTeamManageData } from "@/components/settings/team-manage-detail/useTeamManageData";
import { useTeamManageScrollRestoration } from "@/components/settings/team-manage-detail/useTeamManageScrollRestoration";
import { useTeamManageTabs } from "@/components/settings/team-manage-detail/useTeamManageTabs";
import { handleApiError } from "@/hooks/useApiError";
import { useConfirmDialog } from "@/hooks/useConfirmDialog";
import { getUserDisplayName } from "@/lib/user";
import { teamService } from "@/services/teamService";
import type { TeamInfo, TeamMemberRole, UserStatus } from "@/types/api";

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
	const [archiveConfirmValue, setArchiveConfirmValue] = useState("");
	const [auditOffset, setAuditOffset] = useState(0);
	const [memberIdentifier, setMemberIdentifier] = useState("");
	const [memberOffset, setMemberOffset] = useState(0);
	const [memberQuery, setMemberQuery] = useState("");
	const [memberRole, setMemberRole] = useState<TeamMemberRole>("member");
	const [memberRoleFilter, setMemberRoleFilter] = useState<
		"__all__" | TeamMemberRole
	>("__all__");
	const [memberStatusFilter, setMemberStatusFilter] = useState<
		"__all__" | UserStatus
	>("__all__");
	const [mutating, setMutating] = useState(false);
	const [teamDescription, setTeamDescription] = useState("");
	const [teamName, setTeamName] = useState("");
	const roleLabel = (role: TeamMemberRole) =>
		t(`settings:settings_team_role_${role}`);
	const memberKeyword = memberQuery.trim();
	const memberRoleValue =
		memberRoleFilter === "__all__" ? undefined : memberRoleFilter;
	const memberStatusValue =
		memberStatusFilter === "__all__" ? undefined : memberStatusFilter;
	const memberFilters = useMemo(
		() => ({
			keyword: memberKeyword || undefined,
			role: memberRoleValue,
			status: memberStatusValue,
		}),
		[memberKeyword, memberRoleValue, memberStatusValue],
	);
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
		memberFilters,
		memberOffset,
		open,
		teamId,
		teamSummary,
	});
	const { contentRef, handleContentScroll, handleSidebarScroll, sidebarRef } =
		useTeamManageScrollRestoration({
			isPageLayout,
			pageTab,
			teamId,
		});
	const { currentTab, handleTabChange, panelAnimationClass, resetDialogTab } =
		useTeamManageTabs({
			canArchiveTeam,
			canManageTeam,
			detailLoading,
			detailRequestStarted,
			isPageLayout,
			onPageTabChange,
			pageTab,
		});
	const roleOptions: TeamMemberRole[] = canAssignOwner
		? ["owner", "admin", "member"]
		: ["admin", "member"];
	const quota = displayTeam?.storage_quota ?? 0;
	const used = displayTeam?.storage_used ?? 0;
	const usagePercentage = quota > 0 ? Math.min((used / quota) * 100, 100) : 0;
	const statusFilterOptions = [
		{
			label: t("settings:settings_team_member_status_filter_all"),
			value: "__all__",
		},
		{ label: t("core:active"), value: "active" },
		{ label: t("core:disabled_status"), value: "disabled" },
	] satisfies ReadonlyArray<{
		label: string;
		value: "__all__" | UserStatus;
	}>;
	const roleFilterOptions = [
		{
			label: t("settings:settings_team_member_role_filter_all"),
			value: "__all__",
		},
		...roleOptions.map((role) => ({
			label: roleLabel(role),
			value: role,
		})),
	] satisfies ReadonlyArray<{
		label: string;
		value: "__all__" | TeamMemberRole;
	}>;

	useEffect(() => {
		setArchiveConfirmValue("");
		setTeamName(displayTeam?.name ?? "");
		setTeamDescription(displayTeam?.description ?? "");
	}, [displayTeam?.description, displayTeam?.name]);

	const hasMemberFilters =
		memberKeyword.length > 0 ||
		memberRoleFilter !== "__all__" ||
		memberStatusFilter !== "__all__";
	const memberTotalPages = Math.max(
		1,
		Math.ceil(memberTotal / TEAM_MANAGE_MEMBER_PAGE_SIZE),
	);
	const memberCurrentPage =
		Math.floor(memberOffset / TEAM_MANAGE_MEMBER_PAGE_SIZE) + 1;
	const prevMemberPageDisabled = memberOffset === 0;
	const nextMemberPageDisabled =
		memberOffset + TEAM_MANAGE_MEMBER_PAGE_SIZE >= memberTotal;
	const auditTotalPages = Math.max(
		1,
		Math.ceil(auditTotal / TEAM_MANAGE_AUDIT_PAGE_SIZE),
	);
	const auditCurrentPage =
		Math.floor(auditOffset / TEAM_MANAGE_AUDIT_PAGE_SIZE) + 1;
	const prevAuditPageDisabled = auditOffset === 0;
	const nextAuditPageDisabled =
		auditOffset + TEAM_MANAGE_AUDIT_PAGE_SIZE >= auditTotal;

	useEffect(() => {
		if (memberOffset < memberTotal || memberTotal === 0) {
			return;
		}

		setMemberOffset(
			Math.max(0, (memberTotalPages - 1) * TEAM_MANAGE_MEMBER_PAGE_SIZE),
		);
	}, [memberOffset, memberTotal, memberTotalPages]);

	const handleUpdateTeam = async (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (!teamDetail || !canManageTeam) {
			return;
		}

		const nextName = teamName.trim();
		if (!nextName) {
			return;
		}

		try {
			setMutating(true);
			await teamService.update(teamDetail.id, {
				name: nextName,
				description: teamDescription.trim() || undefined,
			});
			await Promise.all([
				loadTeamDetail(teamDetail.id),
				canManageTeam ? loadAuditEntries(teamDetail.id) : Promise.resolve(),
				onTeamsReload(),
			]);
			toast.success(t("settings:settings_team_updated"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setMutating(false);
		}
	};

	const handleAddMember = async (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (teamId == null || !canManageTeam) {
			return;
		}

		const identifier = memberIdentifier.trim();
		if (!identifier) {
			return;
		}

		try {
			setMutating(true);
			await teamService.addMember(teamId, {
				identifier,
				role: memberRole,
			});
			setMemberIdentifier("");
			setMemberRole("member");
			setMemberOffset(0);
			await Promise.all([
				loadTeamDetail(teamId),
				loadMembers(teamId, 0),
				loadAuditEntries(teamId),
				onTeamsReload(),
			]);
			toast.success(t("settings:settings_team_member_added"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setMutating(false);
		}
	};

	const handleUpdateMemberRole = async (
		memberUserId: number,
		role: TeamMemberRole,
	) => {
		if (teamId == null || !canManageTeam) {
			return;
		}

		try {
			setMutating(true);
			await teamService.updateMember(teamId, memberUserId, { role });
			await Promise.all([
				loadTeamDetail(teamId),
				loadMembers(teamId, memberOffset),
				loadAuditEntries(teamId),
			]);
			toast.success(t("settings:settings_team_member_role_updated"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setMutating(false);
		}
	};

	const handleRemoveMember = async (memberUserId: number) => {
		if (teamId == null) {
			return;
		}

		const removingSelf = memberUserId === currentUserId;

		try {
			setMutating(true);
			await teamService.removeMember(teamId, memberUserId);
			await onTeamsReload();
			if (removingSelf) {
				onOpenChange(false);
				toast.success(t("settings:settings_team_left"));
			} else {
				await Promise.all([
					loadTeamDetail(teamId),
					loadMembers(teamId, memberOffset),
					loadAuditEntries(teamId),
				]);
				toast.success(t("settings:settings_team_member_removed"));
			}
		} catch (error) {
			handleApiError(error);
		} finally {
			setMutating(false);
		}
	};

	const handleArchiveTeam = async () => {
		if (teamId == null || !canArchiveTeam) {
			return;
		}

		try {
			setMutating(true);
			await teamService.delete(teamId);
			await Promise.all([onTeamsReload(), onArchivedReload()]);
			archiveDialogProps.onOpenChange(false);
			onOpenChange(false);
			toast.success(t("settings:settings_team_deleted"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setMutating(false);
		}
	};

	const {
		confirmId: removeMemberId,
		requestConfirm: requestRemoveConfirm,
		dialogProps: removeDialogProps,
	} = useConfirmDialog(handleRemoveMember);
	const {
		requestConfirm: requestArchiveConfirm,
		dialogProps: archiveDialogProps,
	} = useConfirmDialog<true>(handleArchiveTeam);

	useEffect(() => {
		if (!open || teamId == null) {
			setArchiveConfirmValue("");
			archiveDialogProps.onOpenChange(false);
			setAuditOffset(0);
			setMemberIdentifier("");
			setMemberOffset(0);
			setMemberQuery("");
			setMemberRole("member");
			setMemberRoleFilter("__all__");
			setMemberStatusFilter("__all__");
			setMutating(false);
			setTeamDescription("");
			setTeamName("");
			resetDialogTab();
			return;
		}

		setAuditOffset(0);
		setMemberOffset(0);
		resetDialogTab();
	}, [archiveDialogProps.onOpenChange, open, resetDialogTab, teamId]);

	const removeMember =
		members.find((member) => member.user_id === removeMemberId) ?? null;

	if (teamId == null) {
		return null;
	}

	const handleDialogOpenChange = (nextOpen: boolean) => {
		if (!nextOpen) {
			archiveDialogProps.onOpenChange(false);
		}
		onOpenChange(nextOpen);
	};
	const handleArchiveDialogOpenChange = (nextOpen: boolean) => {
		if (nextOpen) {
			requestArchiveConfirm(true);
			return;
		}
		archiveDialogProps.onOpenChange(false);
	};

	const overviewSection = (
		<TeamManageOverviewSection
			canManageTeam={canManageTeam}
			detailLoading={detailLoading}
			mutating={mutating}
			onDescriptionChange={setTeamDescription}
			onSubmit={(event) => void handleUpdateTeam(event)}
			onTeamNameChange={setTeamName}
			team={displayTeam}
			teamDescription={teamDescription}
			teamName={teamName}
		/>
	);

	const membersSection = (
		<TeamManageMembersSection
			canAssignOwner={canAssignOwner}
			canManageTeam={canManageTeam}
			currentUserId={currentUserId}
			hasMemberFilters={hasMemberFilters}
			managerCount={managerCount}
			memberCurrentPage={memberCurrentPage}
			memberIdentifier={memberIdentifier}
			memberLoading={memberLoading}
			memberOffset={memberOffset}
			memberPageSize={TEAM_MANAGE_MEMBER_PAGE_SIZE}
			memberQuery={memberQuery}
			memberRole={memberRole}
			memberRoleFilter={memberRoleFilter}
			memberStatusFilter={memberStatusFilter}
			memberTotal={memberTotal}
			memberTotalPages={memberTotalPages}
			members={members}
			mutating={mutating}
			nextMemberPageDisabled={nextMemberPageDisabled}
			onAddMember={(event) => void handleAddMember(event)}
			onUpdateMemberRole={handleUpdateMemberRole}
			ownerCount={ownerCount}
			prevMemberPageDisabled={prevMemberPageDisabled}
			requestRemoveConfirm={requestRemoveConfirm}
			roleFilterOptions={roleFilterOptions}
			roleLabel={roleLabel}
			roleOptions={roleOptions}
			setMemberIdentifier={setMemberIdentifier}
			setMemberOffset={setMemberOffset}
			setMemberQuery={setMemberQuery}
			setMemberRole={setMemberRole}
			setMemberRoleFilter={setMemberRoleFilter}
			setMemberStatusFilter={setMemberStatusFilter}
			statusFilterOptions={statusFilterOptions}
			team={displayTeam}
			viewerRole={viewerRole}
		/>
	);

	const auditSection = canManageTeam ? (
		<TeamManageAuditSection
			auditCurrentPage={auditCurrentPage}
			auditEntries={auditEntries}
			auditLoading={auditLoading}
			auditOffset={auditOffset}
			auditPageSize={TEAM_MANAGE_AUDIT_PAGE_SIZE}
			auditTotal={auditTotal}
			auditTotalPages={auditTotalPages}
			nextAuditPageDisabled={nextAuditPageDisabled}
			prevAuditPageDisabled={prevAuditPageDisabled}
			roleLabel={roleLabel}
			setAuditOffset={setAuditOffset}
		/>
	) : null;

	const dangerSection = canArchiveTeam ? (
		<TeamManageDangerSection
			archiveConfirmValue={archiveConfirmValue}
			managerCount={managerCount}
			mutating={mutating}
			ownerCount={ownerCount}
			setArchiveConfirmValue={setArchiveConfirmValue}
			setArchiveDialogOpen={handleArchiveDialogOpenChange}
			team={displayTeam}
		/>
	) : null;

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
				onContentScroll={handleContentScroll}
				onOpenChange={handleDialogOpenChange}
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
			/>

			<ConfirmDialog
				{...removeDialogProps}
				title={
					removeMember?.user_id === currentUserId
						? t("settings:settings_team_leave")
						: t("settings:settings_team_remove_member")
				}
				description={
					removeMember
						? `${t("settings:settings_team_remove_member_desc")} ${getUserDisplayName(removeMember.user)}`
						: t("settings:settings_team_remove_member_desc")
				}
				confirmLabel={
					removeMember?.user_id === currentUserId
						? t("settings:settings_team_leave")
						: t("settings:settings_team_remove_member")
				}
				variant="destructive"
			/>

			<ConfirmDialog
				{...archiveDialogProps}
				title={t("settings:settings_team_archive")}
				description={t("settings:settings_team_archive_desc")}
				confirmLabel={t("settings:settings_team_archive")}
				variant="destructive"
			/>
		</>
	);
}
