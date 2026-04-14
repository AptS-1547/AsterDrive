import {
	type FormEvent,
	type ReactNode,
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
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { handleApiError } from "@/hooks/useApiError";
import { useConfirmDialog } from "@/hooks/useConfirmDialog";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import { formatBytes, formatDateAbsolute, formatDateShort } from "@/lib/format";
import {
	formatTeamAuditSummary,
	getTeamRoleBadgeClass,
	isTeamManager,
	isTeamOwner,
} from "@/lib/team";
import { cn } from "@/lib/utils";
import { teamService } from "@/services/teamService";
import type {
	TeamAuditEntryInfo,
	TeamInfo,
	TeamMemberInfo,
	TeamMemberRole,
	UserStatus,
} from "@/types/api";

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

export type TeamManageTab = "overview" | "members" | "audit" | "danger";

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
	const [memberTotal, setMemberTotal] = useState(0);
	const [members, setMembers] = useState<TeamMemberInfo[]>([]);
	const [managerCount, setManagerCount] = useState(0);
	const [memberStatusFilter, setMemberStatusFilter] = useState<
		"__all__" | UserStatus
	>("__all__");
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
	const viewerRole = teamDetail?.my_role ?? teamSummary?.my_role ?? null;
	const canManageTeam = isTeamManager(viewerRole);
	const canAssignOwner = isTeamOwner(viewerRole);
	const canArchiveTeam = isTeamOwner(viewerRole);
	const roleOptions: TeamMemberRole[] = canAssignOwner
		? ["owner", "admin", "member"]
		: ["admin", "member"];
	const quota = teamDetail?.storage_quota ?? teamSummary?.storage_quota ?? 0;
	const used = teamDetail?.storage_used ?? teamSummary?.storage_used ?? 0;
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
			setMembers([]);
			setMemberTotal(0);
			setManagerCount(0);
			setMemberStatusFilter("__all__");
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
		setTeamName(teamDetail?.name ?? teamSummary?.name ?? "");
		setTeamDescription(
			teamDetail?.description ?? teamSummary?.description ?? "",
		);
	}, [
		teamDetail?.description,
		teamDetail?.name,
		teamSummary?.description,
		teamSummary?.name,
	]);

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

	const Wrapper = ({ children }: { children: ReactNode }) =>
		isPageLayout ? (
			<div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-3xl border bg-background shadow-xs">
				{children}
			</div>
		) : (
			<Dialog
				open={open}
				onOpenChange={(nextOpen) => {
					if (!nextOpen) {
						setArchiveDialogOpen(false);
					}
					onOpenChange(nextOpen);
				}}
			>
				<DialogContent className="flex max-h-[min(860px,calc(100vh-2rem))] flex-col gap-0 overflow-hidden p-0 sm:max-w-[min(1120px,calc(100vw-2rem))]">
					{children}
				</DialogContent>
			</Dialog>
		);

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

	const overviewSection = (
		<section className="rounded-2xl border bg-background/60 p-6">
			<div className="mb-5">
				<h4 className="text-base font-semibold text-foreground">
					{t("settings:settings_team_details")}
				</h4>
				<p className="mt-1 text-sm text-muted-foreground">
					{t("settings:settings_team_details_desc")}
				</p>
			</div>
			{detailLoading && !teamDetail ? (
				<SkeletonTable columns={2} rows={4} />
			) : (
				<form
					className="space-y-4"
					onSubmit={(event) => void handleUpdateTeam(event)}
				>
					<div className="space-y-2">
						<Label htmlFor="team-manage-name">{t("core:name")}</Label>
						<Input
							id="team-manage-name"
							value={teamName}
							maxLength={128}
							readOnly={!canManageTeam}
							disabled={mutating || detailLoading}
							className={ADMIN_CONTROL_HEIGHT_CLASS}
							onChange={(event) => setTeamName(event.target.value)}
						/>
					</div>
					<div className="space-y-2">
						<Label htmlFor="team-manage-description">
							{t("settings:settings_team_description")}
						</Label>
						<textarea
							id="team-manage-description"
							value={teamDescription}
							readOnly={!canManageTeam}
							disabled={mutating || detailLoading}
							rows={5}
							className="min-h-28 w-full rounded-lg border border-input bg-transparent px-3 py-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:bg-input/50"
							onChange={(event) => setTeamDescription(event.target.value)}
						/>
					</div>
					<div className="flex flex-wrap items-center justify-between gap-3 border-t pt-4">
						<p className="text-xs text-muted-foreground">
							{detailLoading
								? t("core:loading")
								: t("settings:settings_team_dialog_hint")}
						</p>
						{canManageTeam ? (
							<Button
								type="submit"
								disabled={mutating || detailLoading || !teamName.trim()}
							>
								{t("core:save")}
							</Button>
						) : null}
					</div>
				</form>
			)}
		</section>
	);

	const membersSection = (
		<section className="rounded-2xl border bg-background/60 p-6">
			<div className="mb-5 flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
				<div>
					<h4 className="text-base font-semibold text-foreground">
						{t("settings:settings_team_members")}
					</h4>
					<p className="mt-1 text-sm text-muted-foreground">
						{t("settings:settings_team_members_desc")}
					</p>
				</div>
				<div className="grid gap-2 sm:grid-cols-[minmax(220px,1fr)_160px_160px]">
					<Input
						value={memberQuery}
						onChange={(event) => {
							setMemberOffset(0);
							setMemberQuery(event.target.value);
						}}
						placeholder={t("settings:settings_team_member_search_placeholder")}
						className={ADMIN_CONTROL_HEIGHT_CLASS}
					/>
					<Select
						items={roleFilterOptions}
						value={memberRoleFilter}
						onValueChange={(value) => {
							setMemberOffset(0);
							setMemberRoleFilter(
								(value as "__all__" | TeamMemberRole) ?? "__all__",
							);
						}}
					>
						<SelectTrigger>
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							{roleFilterOptions.map((option) => (
								<SelectItem key={option.value} value={option.value}>
									{option.label}
								</SelectItem>
							))}
						</SelectContent>
					</Select>
					<Select
						items={statusFilterOptions}
						value={memberStatusFilter}
						onValueChange={(value) => {
							setMemberOffset(0);
							setMemberStatusFilter(
								(value as "__all__" | UserStatus) ?? "__all__",
							);
						}}
					>
						<SelectTrigger>
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							{statusFilterOptions.map((option) => (
								<SelectItem key={option.value} value={option.value}>
									{option.label}
								</SelectItem>
							))}
						</SelectContent>
					</Select>
				</div>
			</div>

			<div className="mb-4 flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-muted/20 px-4 py-3 text-sm">
				<div className="flex flex-wrap gap-4 text-muted-foreground">
					<span>
						{t("settings:settings_team_member_filtered_count", {
							filtered: memberTotal,
							total:
								teamDetail?.member_count ??
								teamSummary?.member_count ??
								memberTotal,
						})}
					</span>
					<span>
						{t("settings:settings_team_owner_count")}: {ownerCount}
					</span>
					<span>
						{t("settings:settings_team_manager_count")}: {managerCount}
					</span>
				</div>
				{hasMemberFilters ? (
					<Button
						type="button"
						variant="ghost"
						size="sm"
						onClick={() => {
							setMemberOffset(0);
							setMemberQuery("");
							setMemberRoleFilter("__all__");
							setMemberStatusFilter("__all__");
						}}
					>
						{t("settings:settings_team_clear_filters")}
					</Button>
				) : null}
			</div>

			{canManageTeam ? (
				<form
					className="mb-4 grid gap-3 rounded-xl border bg-muted/20 p-4 md:grid-cols-[minmax(0,1fr)_180px_auto]"
					onSubmit={(event) => void handleAddMember(event)}
				>
					<div className="space-y-2">
						<Label htmlFor="team-member-identifier">
							{t("settings:settings_team_member_identifier")}
						</Label>
						<Input
							id="team-member-identifier"
							value={memberIdentifier}
							disabled={mutating}
							placeholder={t("settings:settings_team_member_placeholder")}
							onChange={(event) => setMemberIdentifier(event.target.value)}
						/>
						<p className="text-xs text-muted-foreground">
							{t("settings:settings_team_member_identifier_desc")}
						</p>
					</div>
					<div className="space-y-2">
						<Label>{t("settings:settings_team_role_label")}</Label>
						<Select
							items={roleOptions.map((role) => ({
								label: roleLabel(role),
								value: role,
							}))}
							value={memberRole}
							onValueChange={(value) => setMemberRole(value as TeamMemberRole)}
						>
							<SelectTrigger>
								<SelectValue />
							</SelectTrigger>
							<SelectContent>
								{roleOptions.map((role) => (
									<SelectItem key={role} value={role}>
										{roleLabel(role)}
									</SelectItem>
								))}
							</SelectContent>
						</Select>
					</div>
					<div className="flex items-end">
						<Button
							type="submit"
							className="w-full"
							disabled={mutating || !memberIdentifier.trim()}
						>
							{t("settings:settings_team_add_member")}
						</Button>
					</div>
				</form>
			) : null}

			{memberLoading && members.length === 0 ? (
				<SkeletonTable columns={6} rows={5} />
			) : memberTotal === 0 ? (
				<EmptyState
					icon={<Icon name="ListBullets" className="h-10 w-10" />}
					title={
						hasMemberFilters
							? t("settings:settings_team_member_filtered_empty")
							: t("settings:settings_team_no_members")
					}
					description={
						hasMemberFilters
							? t("settings:settings_team_member_filtered_empty_desc")
							: t("settings:settings_team_no_members_desc")
					}
				/>
			) : (
				<>
					<div className="overflow-x-auto rounded-xl border">
						<Table>
							<TableHeader>
								<TableRow>
									<TableHead>{t("settings:settings_team_member")}</TableHead>
									<TableHead>{t("settings:settings_team_email")}</TableHead>
									<TableHead>{t("settings:settings_team_status")}</TableHead>
									<TableHead>
										{t("settings:settings_team_role_label")}
									</TableHead>
									<TableHead>{t("core:created_at")}</TableHead>
									<TableHead>{t("core:actions")}</TableHead>
								</TableRow>
							</TableHeader>
							<TableBody>
								{members.map((member) => {
									const isSelf = member.user_id === currentUserId;
									const canRemoveSelf = isSelf && !isTeamOwner(viewerRole);
									const canManageOwner =
										canAssignOwner || member.role !== "owner";
									const canEditRole =
										canManageTeam && canManageOwner && !mutating;
									const canRemove =
										(canManageTeam && canManageOwner) || canRemoveSelf;

									return (
										<TableRow key={member.id}>
											<TableCell>
												<div className="space-y-1">
													<div className="flex items-center gap-2">
														<span className="font-medium">
															{member.username}
														</span>
														{isSelf ? (
															<Badge variant="outline">
																{t("settings:settings_team_you")}
															</Badge>
														) : null}
														{!canEditRole ? (
															<Badge
																className={cn(
																	"border",
																	getTeamRoleBadgeClass(member.role),
																)}
															>
																{roleLabel(member.role)}
															</Badge>
														) : null}
													</div>
													<p className="text-xs text-muted-foreground">
														#{member.user_id}
													</p>
												</div>
											</TableCell>
											<TableCell>{member.email}</TableCell>
											<TableCell>
												<Badge
													variant="outline"
													className={
														member.status === "active"
															? "border-green-500/60 bg-green-500/10 text-green-700 dark:text-green-300"
															: "border-amber-500/60 bg-amber-500/10 text-amber-700 dark:text-amber-300"
													}
												>
													{member.status === "active"
														? t("core:active")
														: t("core:disabled_status")}
												</Badge>
											</TableCell>
											<TableCell>
												{canEditRole ? (
													<Select
														items={roleOptions.map((role) => ({
															label: roleLabel(role),
															value: role,
														}))}
														value={member.role}
														onValueChange={(value) => {
															if (value && value !== member.role) {
																void handleUpdateMemberRole(
																	member.user_id,
																	value as TeamMemberRole,
																);
															}
														}}
													>
														<SelectTrigger width="compact">
															<SelectValue />
														</SelectTrigger>
														<SelectContent>
															{roleOptions.map((role) => (
																<SelectItem key={role} value={role}>
																	{roleLabel(role)}
																</SelectItem>
															))}
														</SelectContent>
													</Select>
												) : (
													<span className="text-sm text-muted-foreground">
														{roleLabel(member.role)}
													</span>
												)}
											</TableCell>
											<TableCell className="text-sm text-muted-foreground">
												{formatDateShort(member.created_at)}
											</TableCell>
											<TableCell>
												{canRemove ? (
													<Button
														type="button"
														variant="ghost"
														size="sm"
														className="text-destructive"
														disabled={mutating}
														onClick={() => requestRemoveConfirm(member.user_id)}
													>
														{isSelf
															? t("settings:settings_team_leave")
															: t("settings:settings_team_remove_member")}
													</Button>
												) : (
													<span className="text-xs text-muted-foreground">
														-
													</span>
												)}
											</TableCell>
										</TableRow>
									);
								})}
							</TableBody>
						</Table>
					</div>
					{memberTotal > MEMBER_PAGE_SIZE ? (
						<div className="mt-4 flex items-center justify-between gap-3 text-sm text-muted-foreground">
							<span>
								{t("settings:settings_team_entries_page", {
									total: memberTotal,
									current: memberCurrentPage,
									pages: memberTotalPages,
								})}
							</span>
							<div className="flex items-center gap-2">
								<Button
									type="button"
									variant="outline"
									size="sm"
									disabled={prevMemberPageDisabled || memberLoading}
									onClick={() =>
										setMemberOffset(
											Math.max(0, memberOffset - MEMBER_PAGE_SIZE),
										)
									}
								>
									<Icon name="CaretLeft" className="h-4 w-4" />
								</Button>
								<Button
									type="button"
									variant="outline"
									size="sm"
									disabled={nextMemberPageDisabled || memberLoading}
									onClick={() =>
										setMemberOffset(memberOffset + MEMBER_PAGE_SIZE)
									}
								>
									<Icon name="CaretRight" className="h-4 w-4" />
								</Button>
							</div>
						</div>
					) : null}
				</>
			)}
		</section>
	);

	const auditSection = canManageTeam ? (
		<section className="rounded-2xl border bg-background/60 p-6">
			<div className="mb-5">
				<h4 className="text-base font-semibold text-foreground">
					{t("settings:settings_team_audit_title")}
				</h4>
				<p className="mt-1 text-sm text-muted-foreground">
					{t("settings:settings_team_audit_desc")}
				</p>
			</div>
			{auditLoading && auditEntries.length === 0 ? (
				<SkeletonTable columns={4} rows={4} />
			) : auditTotal === 0 ? (
				<EmptyState
					icon={<Icon name="Scroll" className="h-10 w-10" />}
					title={t("settings:settings_team_audit_empty")}
					description={t("settings:settings_team_audit_empty_desc")}
				/>
			) : (
				<>
					<div className="space-y-3">
						{auditEntries.map((entry) => {
							const summary = formatTeamAuditSummary(entry, roleLabel);

							return (
								<div
									key={entry.id}
									className="rounded-xl border bg-muted/10 p-4"
								>
									<div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
										<div className="space-y-2">
											<div className="flex flex-wrap items-center gap-2">
												<Badge variant="outline">
													{t(`settings:${entry.action}`, {
														defaultValue: entry.action,
													})}
												</Badge>
												<span className="text-sm text-foreground">
													@{entry.actor_username}
												</span>
											</div>
											<p className="text-sm text-muted-foreground">
												{formatDateAbsolute(entry.created_at)}
											</p>
											{summary ? (
												<p className="text-sm text-muted-foreground">
													{summary}
												</p>
											) : null}
										</div>
									</div>
								</div>
							);
						})}
					</div>
					{auditTotal > AUDIT_PAGE_SIZE ? (
						<div className="mt-4 flex items-center justify-between gap-3 text-sm text-muted-foreground">
							<span>
								{t("settings:settings_team_entries_page", {
									total: auditTotal,
									current: auditCurrentPage,
									pages: auditTotalPages,
								})}
							</span>
							<div className="flex items-center gap-2">
								<Button
									type="button"
									variant="outline"
									size="sm"
									disabled={prevAuditPageDisabled || auditLoading}
									onClick={() =>
										setAuditOffset(Math.max(0, auditOffset - AUDIT_PAGE_SIZE))
									}
								>
									<Icon name="CaretLeft" className="h-4 w-4" />
								</Button>
								<Button
									type="button"
									variant="outline"
									size="sm"
									disabled={nextAuditPageDisabled || auditLoading}
									onClick={() => setAuditOffset(auditOffset + AUDIT_PAGE_SIZE)}
								>
									<Icon name="CaretRight" className="h-4 w-4" />
								</Button>
							</div>
						</div>
					) : null}
				</>
			)}
		</section>
	) : null;

	const dangerSection = canArchiveTeam ? (
		<section className="rounded-2xl border border-destructive/30 bg-destructive/5 p-6">
			<div className="mb-5">
				<h4 className="text-base font-semibold text-foreground">
					{t("settings:settings_team_danger_zone")}
				</h4>
				<p className="mt-1 text-sm text-muted-foreground">
					{t("settings:settings_team_danger_zone_desc")}
				</p>
			</div>
			<div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
				<div className="space-y-3 rounded-xl border bg-background/70 p-4">
					<div className="flex items-center justify-between gap-3">
						<span className="text-sm text-muted-foreground">
							{t("settings:settings_team_owner_count")}
						</span>
						<span className="font-medium">{ownerCount}</span>
					</div>
					<div className="flex items-center justify-between gap-3">
						<span className="text-sm text-muted-foreground">
							{t("settings:settings_team_manager_count")}
						</span>
						<span className="font-medium">{managerCount}</span>
					</div>
					<div className="flex items-center justify-between gap-3">
						<span className="text-sm text-muted-foreground">
							{t("settings:settings_team_status")}
						</span>
						<span className="font-medium">{t("core:active")}</span>
					</div>
					<p className="text-xs text-muted-foreground">
						{t("settings:settings_team_danger_zone_hint")}
					</p>
				</div>
				<div className="space-y-3 rounded-xl border border-destructive/30 bg-background/70 p-4">
					<div className="space-y-2">
						<Label htmlFor="team-archive-confirm">
							{t("settings:settings_team_archive_confirm_label")}
						</Label>
						<Input
							id="team-archive-confirm"
							value={archiveConfirmValue}
							placeholder={t(
								"settings:settings_team_archive_confirm_placeholder",
							)}
							onChange={(event) => setArchiveConfirmValue(event.target.value)}
							className={ADMIN_CONTROL_HEIGHT_CLASS}
						/>
						<p className="text-xs text-muted-foreground">
							{t("settings:settings_team_archive_confirm_hint", {
								name: teamDetail?.name ?? teamSummary?.name ?? "",
							})}
						</p>
					</div>
					<Button
						type="button"
						variant="destructive"
						disabled={
							mutating ||
							archiveConfirmValue.trim() !==
								(teamDetail?.name ?? teamSummary?.name ?? "")
						}
						onClick={() => setArchiveDialogOpen(true)}
					>
						{t("settings:settings_team_archive")}
					</Button>
				</div>
			</div>
		</section>
	) : null;

	return (
		<>
			{Wrapper({
				children: (
					<>
						{isPageLayout ? (
							<div className="flex flex-wrap items-start justify-between gap-3 border-b px-6 pt-5 pb-4">
								<div className="space-y-1">
									<p className="text-xs uppercase tracking-wide text-muted-foreground">
										{t("settings:settings_teams")}
									</p>
									<h1 className="text-xl font-semibold tracking-tight">
										{teamDetail?.name ??
											teamSummary?.name ??
											t("settings:settings_team_manage_title")}
									</h1>
									<p className="text-sm text-muted-foreground">
										{t("settings:settings_team_manage_title")}
									</p>
								</div>
								<Button
									type="button"
									variant="outline"
									onClick={() => onOpenChange(false)}
								>
									<Icon name="CaretLeft" className="mr-1 h-4 w-4" />
									{t("core:back")}
								</Button>
							</div>
						) : (
							<DialogHeader className="flex items-center justify-center px-6 pt-5 pb-0 text-center max-lg:px-4 max-lg:pt-4">
								<DialogTitle className="text-lg">
									{t("settings:settings_team_manage_title")}
								</DialogTitle>
							</DialogHeader>
						)}
						<div
							ref={contentRef}
							className="flex min-h-0 flex-1 flex-col overflow-y-auto lg:overflow-hidden"
							onScroll={() => {
								if (teamId == null || contentRef.current == null) {
									return;
								}

								teamManageContentScrollPositions.set(
									teamId,
									contentRef.current.scrollTop,
								);
							}}
						>
							<div className="flex min-h-full flex-col lg:h-full lg:min-h-0 lg:flex-1 lg:flex-row">
								<aside
									ref={sidebarRef}
									className="border-b bg-muted/20 lg:min-h-0 lg:w-80 lg:flex-none lg:overflow-y-auto lg:border-r lg:border-b-0"
									onScroll={() => {
										if (teamId == null || sidebarRef.current == null) {
											return;
										}

										teamManageSidebarScrollPositions.set(
											teamId,
											sidebarRef.current.scrollTop,
										);
									}}
								>
									<div className="space-y-5 p-6 max-lg:space-y-4 max-lg:p-4">
										<div className="space-y-3 max-lg:flex max-lg:items-start max-lg:gap-3 max-lg:space-y-0">
											<div className="flex size-16 items-center justify-center rounded-2xl bg-primary/10 text-primary max-lg:size-12 max-lg:rounded-xl">
												<Icon name="Cloud" className="h-7 w-7" />
											</div>
											<div className="space-y-3 max-lg:min-w-0 max-lg:flex-1">
												<div className="space-y-1">
													<h3 className="text-lg font-semibold text-foreground">
														{teamDetail?.name ??
															teamSummary?.name ??
															t("core:loading")}
													</h3>
													<p className="text-sm text-muted-foreground max-lg:line-clamp-2">
														{teamDetail?.description ||
															teamSummary?.description ||
															t("settings:settings_team_no_description")}
													</p>
												</div>
												<div className="flex flex-wrap gap-2">
													{viewerRole ? (
														<Badge
															className={cn(
																"border",
																getTeamRoleBadgeClass(viewerRole),
															)}
														>
															{roleLabel(viewerRole)}
														</Badge>
													) : null}
												</div>
											</div>
										</div>

										<div className="space-y-3 rounded-xl border bg-background/60 p-4 max-lg:grid max-lg:grid-cols-2 max-lg:gap-3 max-lg:space-y-0 max-lg:p-3">
											<div className="space-y-1">
												<p className="text-xs uppercase tracking-wide text-muted-foreground">
													ID
												</p>
												<p className="font-mono text-sm text-foreground">
													{teamDetail?.id ?? teamSummary?.id ?? "-"}
												</p>
											</div>
											<div className="space-y-1">
												<p className="text-xs uppercase tracking-wide text-muted-foreground">
													{t("settings:settings_team_created_by")}
												</p>
												<p className="text-sm text-foreground">
													{teamDetail?.created_by_username ??
														teamSummary?.created_by_username ??
														"-"}
												</p>
											</div>
											<div className="space-y-1">
												<p className="text-xs uppercase tracking-wide text-muted-foreground">
													{t("core:created_at")}
												</p>
												<p className="text-sm text-foreground">
													{teamDetail
														? formatDateAbsolute(teamDetail.created_at)
														: teamSummary
															? formatDateAbsolute(teamSummary.created_at)
															: "-"}
												</p>
											</div>
										</div>

										<div className="space-y-3 rounded-xl border bg-background/60 p-4 max-lg:p-3">
											<div>
												<p className="text-sm font-medium text-foreground">
													{t("settings:settings_team_quota")}
												</p>
												<p className="text-xs text-muted-foreground">
													{formatBytes(used)}
													{quota > 0
														? ` / ${formatBytes(quota)}`
														: ` / ${t("core:unlimited")}`}
												</p>
											</div>
											{quota > 0 ? (
												<Progress value={usagePercentage} className="h-2" />
											) : null}
											<div className="space-y-2 text-xs text-muted-foreground">
												<div className="flex items-center justify-between gap-3">
													<span>
														{t("settings:settings_team_members_count")}
													</span>
													<span>
														{teamDetail?.member_count ??
															teamSummary?.member_count ??
															"-"}
													</span>
												</div>
												<div className="flex items-center justify-between gap-3">
													<span>{t("settings:settings_team_owner_count")}</span>
													<span>{ownerCount}</span>
												</div>
												<div className="flex items-center justify-between gap-3">
													<span>
														{t("settings:settings_team_manager_count")}
													</span>
													<span>{managerCount}</span>
												</div>
											</div>
											<Button
												type="button"
												variant="outline"
												onClick={() =>
													navigate(`/teams/${teamId}`, { viewTransition: true })
												}
											>
												{t("settings:settings_team_open_workspace")}
											</Button>
										</div>
									</div>
								</aside>

								<div
									className={cn(
										"min-h-0 min-w-0 lg:flex-1",
										isPageLayout
											? "lg:flex lg:h-full lg:flex-col lg:overflow-hidden"
											: "lg:overflow-y-auto",
									)}
								>
									{isPageLayout ? (
										<Tabs
											value={currentTab}
											onValueChange={handleTabChange}
											className="flex flex-col lg:h-full lg:min-h-0 lg:flex-1 lg:overflow-hidden"
										>
											<div className="px-6 pt-6 max-lg:px-4 max-lg:pt-4 lg:shrink-0">
												<TabsList
													variant="line"
													className="h-auto w-full gap-5 border-b px-0 pb-2"
												>
													<TabsTrigger
														value="overview"
														className="h-10 min-w-0 rounded-none px-0"
													>
														{t("settings:settings_team_overview")}
													</TabsTrigger>
													<TabsTrigger
														value="members"
														className="h-10 min-w-0 rounded-none px-0"
													>
														{t("settings:settings_team_members")}
													</TabsTrigger>
													{canManageTeam ? (
														<TabsTrigger
															value="audit"
															className="h-10 min-w-0 rounded-none px-0"
														>
															{t("settings:settings_team_audit_title")}
														</TabsTrigger>
													) : null}
													{canArchiveTeam ? (
														<TabsTrigger
															value="danger"
															className="h-10 min-w-0 rounded-none px-0"
														>
															{t("settings:settings_team_danger_zone")}
														</TabsTrigger>
													) : null}
												</TabsList>
											</div>

											<div className="px-6 pt-4 pb-6 max-lg:px-4 max-lg:pt-3 max-lg:pb-4 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
												<TabsContent
													value="overview"
													className={cn(
														"outline-none",
														currentTab === "overview" && panelAnimationClass,
													)}
												>
													{overviewSection}
												</TabsContent>
												<TabsContent
													value="members"
													className={cn(
														"outline-none",
														currentTab === "members" && panelAnimationClass,
													)}
												>
													{membersSection}
												</TabsContent>
												{canManageTeam ? (
													<TabsContent
														value="audit"
														className={cn(
															"outline-none",
															currentTab === "audit" && panelAnimationClass,
														)}
													>
														{auditSection}
													</TabsContent>
												) : null}
												{canArchiveTeam ? (
													<TabsContent
														value="danger"
														className={cn(
															"outline-none",
															currentTab === "danger" && panelAnimationClass,
														)}
													>
														{dangerSection}
													</TabsContent>
												) : null}
											</div>
										</Tabs>
									) : (
										<div className="space-y-4 p-6">
											{overviewSection}
											{membersSection}
											{auditSection}
											{dangerSection}
										</div>
									)}
								</div>
							</div>
						</div>
					</>
				),
			})}

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
