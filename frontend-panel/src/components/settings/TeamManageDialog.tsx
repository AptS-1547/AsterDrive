import {
	type FormEvent,
	useEffect,
	useEffectEvent,
	useLayoutEffect,
	useRef,
	useState,
} from "react";
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
import type { TeamManageTab } from "@/components/settings/team-manage-detail/types";
import { handleApiError } from "@/hooks/useApiError";
import { useConfirmDialog } from "@/hooks/useConfirmDialog";
import { isTeamManager, isTeamOwner } from "@/lib/team";
import { teamService } from "@/services/teamService";
import type {
	TeamAuditEntryInfo,
	TeamInfo,
	TeamMemberInfo,
	TeamMemberRole,
	UserStatus,
} from "@/types/api";

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

const MEMBER_PAGE_SIZE = 10;
const AUDIT_PAGE_SIZE = 10;
const TEAM_MANAGE_TAB_INDEX: Record<TeamManageTab, number> = {
	overview: 0,
	members: 1,
	audit: 2,
	danger: 3,
};
const teamManageContentScrollPositions = new Map<number, number>();
const teamManageSidebarScrollPositions = new Map<number, number>();

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
	const [dialogTab, setDialogTab] = useState<TeamManageTab>("overview");
	const [pageLayoutTab, setPageLayoutTab] = useState<TeamManageTab>(
		pageTab ?? "overview",
	);
	const [tabDirection, setTabDirection] = useState<"forward" | "backward">(
		"forward",
	);
	const [archiveConfirmValue, setArchiveConfirmValue] = useState("");
	const [archiveDialogOpen, setArchiveDialogOpen] = useState(false);
	const [auditEntries, setAuditEntries] = useState<TeamAuditEntryInfo[]>([]);
	const [auditLoading, setAuditLoading] = useState(false);
	const [auditOffset, setAuditOffset] = useState(0);
	const [auditTotal, setAuditTotal] = useState(0);
	const [detailLoading, setDetailLoading] = useState(false);
	const [memberIdentifier, setMemberIdentifier] = useState("");
	const [memberLoading, setMemberLoading] = useState(false);
	const [memberOffset, setMemberOffset] = useState(0);
	const [memberQuery, setMemberQuery] = useState("");
	const [memberRole, setMemberRole] = useState<TeamMemberRole>("member");
	const [memberRoleFilter, setMemberRoleFilter] = useState<
		"__all__" | TeamMemberRole
	>("__all__");
	const [memberStatusFilter, setMemberStatusFilter] = useState<
		"__all__" | UserStatus
	>("__all__");
	const [memberTotal, setMemberTotal] = useState(0);
	const [members, setMembers] = useState<TeamMemberInfo[]>([]);
	const [managerCount, setManagerCount] = useState(0);
	const [mutating, setMutating] = useState(false);
	const [ownerCount, setOwnerCount] = useState(0);
	const [teamDescription, setTeamDescription] = useState("");
	const [teamDetail, setTeamDetail] = useState<TeamInfo | null>(null);
	const [teamName, setTeamName] = useState("");
	const auditRequestIdRef = useRef(0);
	const contentRef = useRef<HTMLDivElement | null>(null);
	const detailRequestIdRef = useRef(0);
	const memberRequestIdRef = useRef(0);
	const sidebarRef = useRef<HTMLElement | null>(null);
	const displayTeam = teamDetail ?? teamSummary;
	const viewerRole = displayTeam?.my_role ?? null;
	const canManageTeam = isTeamManager(viewerRole);
	const canAssignOwner = isTeamOwner(viewerRole);
	const canArchiveTeam = isTeamOwner(viewerRole);
	const roleOptions: TeamMemberRole[] = canAssignOwner
		? ["owner", "admin", "member"]
		: ["admin", "member"];
	const quota = displayTeam?.storage_quota ?? 0;
	const used = displayTeam?.storage_used ?? 0;
	const usagePercentage = quota > 0 ? Math.min((used / quota) * 100, 100) : 0;
	const roleLabel = (role: TeamMemberRole) =>
		t(`settings:settings_team_role_${role}`);
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
	const memberKeyword = memberQuery.trim();
	const memberRoleValue =
		memberRoleFilter === "__all__" ? undefined : memberRoleFilter;
	const memberStatusValue =
		memberStatusFilter === "__all__" ? undefined : memberStatusFilter;

	const loadTeamDetail = useEffectEvent(async (nextTeamId: number) => {
		const requestId = ++detailRequestIdRef.current;
		setDetailLoading(true);
		try {
			const detail = await teamService.get(nextTeamId);
			if (requestId !== detailRequestIdRef.current) {
				return;
			}

			setTeamDetail(detail);
		} catch (error) {
			if (requestId !== detailRequestIdRef.current) {
				return;
			}

			setTeamDetail(null);
			handleApiError(error);
		} finally {
			if (requestId === detailRequestIdRef.current) {
				setDetailLoading(false);
			}
		}
	});

	const loadMembers = useEffectEvent(
		async (
			nextTeamId: number,
			nextOffset = memberOffset,
			nextFilters: {
				keyword?: string;
				role?: TeamMemberRole;
				status?: UserStatus;
			} = {
				keyword: memberKeyword || undefined,
				role: memberRoleValue,
				status: memberStatusValue,
			},
		) => {
			const requestId = ++memberRequestIdRef.current;
			setMemberLoading(true);
			try {
				const page = await teamService.listMembers(nextTeamId, {
					keyword: nextFilters.keyword,
					role: nextFilters.role,
					status: nextFilters.status,
					limit: MEMBER_PAGE_SIZE,
					offset: nextOffset,
				});
				if (requestId !== memberRequestIdRef.current) {
					return;
				}

				setMembers(page.items);
				setMemberTotal(page.total);
				setOwnerCount(page.owner_count);
				setManagerCount(page.manager_count);
			} catch (error) {
				if (requestId !== memberRequestIdRef.current) {
					return;
				}

				setMembers([]);
				setMemberTotal(0);
				setOwnerCount(0);
				setManagerCount(0);
				handleApiError(error);
			} finally {
				if (requestId === memberRequestIdRef.current) {
					setMemberLoading(false);
				}
			}
		},
	);

	const loadAuditEntries = useEffectEvent(
		async (nextTeamId: number, nextOffset = auditOffset) => {
			const requestId = ++auditRequestIdRef.current;
			setAuditLoading(true);
			try {
				const page = await teamService.listAuditLogs(nextTeamId, {
					limit: AUDIT_PAGE_SIZE,
					offset: nextOffset,
				});
				if (requestId !== auditRequestIdRef.current) {
					return;
				}

				setAuditEntries(page.items);
				setAuditTotal(page.total);
			} catch (error) {
				if (requestId !== auditRequestIdRef.current) {
					return;
				}

				setAuditEntries([]);
				setAuditTotal(0);
				handleApiError(error);
			} finally {
				if (requestId === auditRequestIdRef.current) {
					setAuditLoading(false);
				}
			}
		},
	);

	useEffect(() => {
		if (!open || teamId == null) {
			auditRequestIdRef.current += 1;
			detailRequestIdRef.current += 1;
			memberRequestIdRef.current += 1;
			setArchiveConfirmValue("");
			setArchiveDialogOpen(false);
			setAuditEntries([]);
			setAuditLoading(false);
			setAuditOffset(0);
			setAuditTotal(0);
			setDetailLoading(false);
			setMemberIdentifier("");
			setMemberLoading(false);
			setMemberOffset(0);
			setMemberQuery("");
			setMemberRole("member");
			setMemberRoleFilter("__all__");
			setMemberStatusFilter("__all__");
			setMemberTotal(0);
			setMembers([]);
			setManagerCount(0);
			setMutating(false);
			setOwnerCount(0);
			setTeamDescription("");
			setTeamDetail(null);
			setTeamName("");
			setDialogTab("overview");
			return;
		}

		setAuditOffset(0);
		setMemberOffset(0);
		setDialogTab("overview");
		void loadTeamDetail(teamId);
	}, [open, teamId]);

	useLayoutEffect(() => {
		if (!isPageLayout || teamId == null || pageTab == null) {
			return;
		}

		const content = contentRef.current;
		if (content != null) {
			content.scrollTop = teamManageContentScrollPositions.get(teamId) ?? 0;
		}

		const sidebar = sidebarRef.current;
		if (sidebar == null) {
			return () => {
				if (contentRef.current == null) {
					return;
				}

				teamManageContentScrollPositions.set(
					teamId,
					contentRef.current.scrollTop,
				);
			};
		}

		sidebar.scrollTop = teamManageSidebarScrollPositions.get(teamId) ?? 0;

		return () => {
			if (contentRef.current != null) {
				teamManageContentScrollPositions.set(
					teamId,
					contentRef.current.scrollTop,
				);
			}

			if (sidebarRef.current == null) {
				return;
			}

			teamManageSidebarScrollPositions.set(
				teamId,
				sidebarRef.current.scrollTop,
			);
		};
	}, [isPageLayout, pageTab, teamId]);

	useEffect(() => {
		if (!open || teamId == null || !canManageTeam) {
			auditRequestIdRef.current += 1;
			setAuditEntries([]);
			setAuditTotal(0);
			setAuditLoading(false);
			return;
		}

		void loadAuditEntries(teamId, auditOffset);
	}, [auditOffset, canManageTeam, open, teamId]);

	useEffect(() => {
		if (!open || teamId == null) {
			memberRequestIdRef.current += 1;
			setMemberLoading(false);
			return;
		}

		void loadMembers(teamId, memberOffset, {
			keyword: memberKeyword || undefined,
			role: memberRoleValue,
			status: memberStatusValue,
		});
	}, [
		memberKeyword,
		memberOffset,
		memberRoleValue,
		memberStatusValue,
		open,
		teamId,
	]);

	useEffect(() => {
		setArchiveConfirmValue("");
		setTeamName(displayTeam?.name ?? "");
		setTeamDescription(displayTeam?.description ?? "");
	}, [displayTeam?.description, displayTeam?.name]);

	useEffect(() => {
		if (
			dialogTab === "overview" ||
			dialogTab === "members" ||
			(dialogTab === "audit" && canManageTeam) ||
			(dialogTab === "danger" && canArchiveTeam)
		) {
			return;
		}

		setDialogTab("overview");
	}, [canArchiveTeam, canManageTeam, dialogTab]);

	const hasMemberFilters =
		memberKeyword.length > 0 ||
		memberRoleFilter !== "__all__" ||
		memberStatusFilter !== "__all__";
	const memberTotalPages = Math.max(
		1,
		Math.ceil(memberTotal / MEMBER_PAGE_SIZE),
	);
	const memberCurrentPage = Math.floor(memberOffset / MEMBER_PAGE_SIZE) + 1;
	const prevMemberPageDisabled = memberOffset === 0;
	const nextMemberPageDisabled = memberOffset + MEMBER_PAGE_SIZE >= memberTotal;
	const auditTotalPages = Math.max(1, Math.ceil(auditTotal / AUDIT_PAGE_SIZE));
	const auditCurrentPage = Math.floor(auditOffset / AUDIT_PAGE_SIZE) + 1;
	const prevAuditPageDisabled = auditOffset === 0;
	const nextAuditPageDisabled = auditOffset + AUDIT_PAGE_SIZE >= auditTotal;

	useEffect(() => {
		if (memberOffset < memberTotal || memberTotal === 0) {
			return;
		}

		setMemberOffset(Math.max(0, (memberTotalPages - 1) * MEMBER_PAGE_SIZE));
	}, [memberOffset, memberTotal, memberTotalPages]);

	useEffect(() => {
		if (!isPageLayout || pageTab == null || pageLayoutTab === pageTab) {
			return;
		}

		setTabDirection(
			TEAM_MANAGE_TAB_INDEX[pageTab] >= TEAM_MANAGE_TAB_INDEX[pageLayoutTab]
				? "forward"
				: "backward",
		);
		setPageLayoutTab(pageTab);
	}, [isPageLayout, pageLayoutTab, pageTab]);

	useEffect(() => {
		if (
			!isPageLayout ||
			pageTab == null ||
			onPageTabChange == null ||
			detailLoading ||
			detailRequestIdRef.current === 0
		) {
			return;
		}

		if (
			pageTab === "overview" ||
			pageTab === "members" ||
			(pageTab === "audit" && canManageTeam) ||
			(pageTab === "danger" && canArchiveTeam)
		) {
			return;
		}

		onPageTabChange("overview", { replace: true });
	}, [
		canArchiveTeam,
		canManageTeam,
		detailLoading,
		isPageLayout,
		onPageTabChange,
		pageTab,
	]);

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
			setArchiveDialogOpen(false);
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
	const removeMember =
		members.find((member) => member.user_id === removeMemberId) ?? null;

	if (teamId == null) {
		return null;
	}

	const currentTab = isPageLayout ? pageLayoutTab : dialogTab;
	const panelAnimationClass =
		tabDirection === "forward"
			? "animate-in fade-in duration-300 slide-in-from-right-4 motion-reduce:animate-none"
			: "animate-in fade-in duration-300 slide-in-from-left-4 motion-reduce:animate-none";

	const handleTabChange = (value: string) => {
		if (
			value === "overview" ||
			value === "members" ||
			value === "audit" ||
			value === "danger"
		) {
			if (isPageLayout) {
				if (value === currentTab) {
					return;
				}

				setTabDirection(
					TEAM_MANAGE_TAB_INDEX[value] >= TEAM_MANAGE_TAB_INDEX[currentTab]
						? "forward"
						: "backward",
				);
				setPageLayoutTab(value);
				onPageTabChange?.(value);
			} else {
				setDialogTab(value);
			}
		}
	};

	const handleDialogOpenChange = (nextOpen: boolean) => {
		if (!nextOpen) {
			setArchiveDialogOpen(false);
		}
		onOpenChange(nextOpen);
	};

	const handleContentScroll = () => {
		if (contentRef.current == null) {
			return;
		}

		teamManageContentScrollPositions.set(teamId, contentRef.current.scrollTop);
	};

	const handleSidebarScroll = () => {
		if (sidebarRef.current == null) {
			return;
		}

		teamManageSidebarScrollPositions.set(teamId, sidebarRef.current.scrollTop);
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
			memberPageSize={MEMBER_PAGE_SIZE}
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
			auditPageSize={AUDIT_PAGE_SIZE}
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
			setArchiveDialogOpen={setArchiveDialogOpen}
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
					navigate(`/teams/${teamId}`, { viewTransition: true })
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
						? `${t("settings:settings_team_remove_member_desc")} @${removeMember.username}`
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
				open={archiveDialogOpen}
				onOpenChange={setArchiveDialogOpen}
				title={t("settings:settings_team_archive")}
				description={t("settings:settings_team_archive_desc")}
				confirmLabel={t("settings:settings_team_archive")}
				onConfirm={() => void handleArchiveTeam()}
				variant="destructive"
			/>
		</>
	);
}
