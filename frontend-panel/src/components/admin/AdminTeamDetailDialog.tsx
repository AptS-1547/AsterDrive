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
import { formatTeamAuditSummary, getTeamRoleBadgeClass } from "@/lib/team";
import { cn } from "@/lib/utils";
import { adminTeamService } from "@/services/adminService";
import type {
	AdminTeamInfo,
	StoragePolicyGroup,
	TeamAuditEntryInfo,
	TeamMemberInfo,
	TeamMemberRole,
	UserStatus,
} from "@/types/api";

interface AdminTeamDetailDialogProps {
	layout?: "dialog" | "page";
	open: boolean;
	teamId: number | null;
	policyGroups: StoragePolicyGroup[];
	policyGroupsLoading: boolean;
	onListChange: () => Promise<void>;
	onOpenChange: (open: boolean) => void;
	onPageTabChange?: (
		tab: AdminTeamDetailTab,
		options?: { replace?: boolean },
	) => void;
	onRefreshPolicyGroups: () => Promise<void>;
	pageTab?: AdminTeamDetailTab;
}

interface PolicyGroupOption {
	disabled?: boolean;
	label: string;
	value: string;
}

export type AdminTeamDetailTab = "overview" | "members" | "audit" | "danger";

const MEMBER_PAGE_SIZE = 10;
const AUDIT_PAGE_SIZE = 10;
const ADMIN_TEAM_DETAIL_TAB_INDEX: Record<AdminTeamDetailTab, number> = {
	overview: 0,
	members: 1,
	audit: 2,
	danger: 3,
};
const adminTeamDetailContentScrollPositions = new Map<number, number>();
const adminTeamDetailSidebarScrollPositions = new Map<number, number>();

function buildPolicyGroupOptions(
	policyGroups: StoragePolicyGroup[],
	selectedPolicyGroupId: number | null,
): PolicyGroupOption[] {
	const options: PolicyGroupOption[] = policyGroups
		.filter((group) => group.is_enabled && group.items.length > 0)
		.map((group) => ({
			label: group.name,
			value: String(group.id),
		}));

	if (
		selectedPolicyGroupId != null &&
		!options.some((option) => option.value === String(selectedPolicyGroupId))
	) {
		const selectedGroup = policyGroups.find(
			(group) => group.id === selectedPolicyGroupId,
		);
		options.unshift({
			label: selectedGroup?.name ?? `#${selectedPolicyGroupId}`,
			value: String(selectedPolicyGroupId),
			disabled: true,
		});
	}

	return options;
}

export function AdminTeamDetailDialog({
	layout = "dialog",
	open,
	teamId,
	policyGroups,
	policyGroupsLoading,
	onListChange,
	onOpenChange,
	onPageTabChange,
	onRefreshPolicyGroups,
	pageTab,
}: AdminTeamDetailDialogProps) {
	const { t } = useTranslation(["admin", "core", "settings"]);
	const isPageLayout = layout === "page";
	const [dialogTab, setDialogTab] = useState<AdminTeamDetailTab>("overview");
	const [pageLayoutTab, setPageLayoutTab] = useState<AdminTeamDetailTab>(
		pageTab ?? "overview",
	);
	const [tabDirection, setTabDirection] = useState<"forward" | "backward">(
		"forward",
	);
	const [archiveConfirmValue, setArchiveConfirmValue] = useState("");
	const [archiveDialogOpen, setArchiveDialogOpen] = useState(false);
	const [archiving, setArchiving] = useState(false);
	const [auditEntries, setAuditEntries] = useState<TeamAuditEntryInfo[]>([]);
	const [auditLoading, setAuditLoading] = useState(false);
	const [auditOffset, setAuditOffset] = useState(0);
	const [auditTotal, setAuditTotal] = useState(0);
	const [detailLoading, setDetailLoading] = useState(false);
	const [memberIdentifier, setMemberIdentifier] = useState("");
	const [memberLoading, setMemberLoading] = useState(false);
	const [memberMutating, setMemberMutating] = useState(false);
	const [memberOffset, setMemberOffset] = useState(0);
	const [memberTotal, setMemberTotal] = useState(0);
	const [members, setMembers] = useState<TeamMemberInfo[]>([]);
	const [managerCount, setManagerCount] = useState(0);
	const [memberQuery, setMemberQuery] = useState("");
	const [memberRole, setMemberRole] = useState<TeamMemberRole>("member");
	const [memberRoleFilter, setMemberRoleFilter] = useState<
		"__all__" | TeamMemberRole
	>("__all__");
	const [memberStatusFilter, setMemberStatusFilter] = useState<
		"__all__" | UserStatus
	>("__all__");
	const [name, setName] = useState("");
	const [ownerCount, setOwnerCount] = useState(0);
	const [description, setDescription] = useState("");
	const [policyGroupId, setPolicyGroupId] = useState("");
	const [restoring, setRestoring] = useState(false);
	const [saving, setSaving] = useState(false);
	const [team, setTeam] = useState<AdminTeamInfo | null>(null);
	const auditRequestIdRef = useRef(0);
	const contentRef = useRef<HTMLDivElement | null>(null);
	const detailRequestIdRef = useRef(0);
	const memberRequestIdRef = useRef(0);
	const sidebarRef = useRef<HTMLElement | null>(null);
	const roleOptions: TeamMemberRole[] = ["owner", "admin", "member"];
	const statusFilterOptions = [
		{ label: t("member_status_filter_all"), value: "__all__" },
		{ label: t("core:active"), value: "active" },
		{ label: t("core:disabled_status"), value: "disabled" },
	] satisfies ReadonlyArray<{
		label: string;
		value: "__all__" | UserStatus;
	}>;
	const roleFilterOptions = [
		{ label: t("member_role_filter_all"), value: "__all__" },
		...roleOptions.map((role) => ({
			label: t(`settings:settings_team_role_${role}`),
			value: role,
		})),
	] satisfies ReadonlyArray<{
		label: string;
		value: "__all__" | TeamMemberRole;
	}>;

	const roleLabel = (role: TeamMemberRole) =>
		t(`settings:settings_team_role_${role}`);
	const memberKeyword = memberQuery.trim();
	const memberRoleValue =
		memberRoleFilter === "__all__" ? undefined : memberRoleFilter;
	const memberStatusValue =
		memberStatusFilter === "__all__" ? undefined : memberStatusFilter;

	const loadTeamDetail = useEffectEvent(async (nextTeamId: number) => {
		const requestId = ++detailRequestIdRef.current;
		setDetailLoading(true);
		try {
			const detail = await adminTeamService.get(nextTeamId);
			if (requestId !== detailRequestIdRef.current) {
				return;
			}
			setTeam(detail);
		} catch (error) {
			if (requestId !== detailRequestIdRef.current) {
				return;
			}
			setTeam(null);
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
				const page = await adminTeamService.listMembers(nextTeamId, {
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
				const page = await adminTeamService.listAuditLogs(nextTeamId, {
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
			setArchiving(false);
			setAuditEntries([]);
			setAuditLoading(false);
			setAuditOffset(0);
			setAuditTotal(0);
			setDescription("");
			setDetailLoading(false);
			setMemberIdentifier("");
			setMemberLoading(false);
			setMemberMutating(false);
			setMemberOffset(0);
			setMemberTotal(0);
			setMembers([]);
			setManagerCount(0);
			setMemberQuery("");
			setMemberRole("member");
			setMemberRoleFilter("__all__");
			setMemberStatusFilter("__all__");
			setName("");
			setOwnerCount(0);
			setPolicyGroupId("");
			setRestoring(false);
			setSaving(false);
			setTeam(null);
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
			content.scrollTop =
				adminTeamDetailContentScrollPositions.get(teamId) ?? 0;
		}

		const sidebar = sidebarRef.current;
		if (sidebar == null) {
			return () => {
				if (contentRef.current == null) {
					return;
				}

				adminTeamDetailContentScrollPositions.set(
					teamId,
					contentRef.current.scrollTop,
				);
			};
		}

		sidebar.scrollTop = adminTeamDetailSidebarScrollPositions.get(teamId) ?? 0;

		return () => {
			if (contentRef.current != null) {
				adminTeamDetailContentScrollPositions.set(
					teamId,
					contentRef.current.scrollTop,
				);
			}

			if (sidebarRef.current == null) {
				return;
			}

			adminTeamDetailSidebarScrollPositions.set(
				teamId,
				sidebarRef.current.scrollTop,
			);
		};
	}, [isPageLayout, pageTab, teamId]);

	useEffect(() => {
		if (!open || teamId == null) {
			auditRequestIdRef.current += 1;
			setAuditLoading(false);
			return;
		}

		void loadAuditEntries(teamId, auditOffset);
	}, [auditOffset, open, teamId]);

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
		setName(team?.name ?? "");
		setDescription(team?.description ?? "");
		setPolicyGroupId(
			team?.policy_group_id != null ? String(team.policy_group_id) : "",
		);
	}, [team]);

	const quota = team?.storage_quota ?? 0;
	const used = team?.storage_used ?? 0;
	const usagePercentage = quota > 0 ? Math.min((used / quota) * 100, 100) : 0;
	const selectedPolicyGroupId = policyGroupId ? Number(policyGroupId) : null;
	const policyGroupOptions = buildPolicyGroupOptions(
		policyGroups,
		selectedPolicyGroupId ?? team?.policy_group_id ?? null,
	);
	const currentPolicyGroup =
		team?.policy_group_id != null
			? (policyGroups.find((group) => group.id === team.policy_group_id) ??
				null)
			: null;
	const selectedPolicyGroup =
		selectedPolicyGroupId != null
			? (policyGroups.find((group) => group.id === selectedPolicyGroupId) ??
				null)
			: null;
	const policyGroupUnavailable =
		!policyGroupsLoading && policyGroupOptions.length === 0;
	const assignedPolicyGroupIsInvalid =
		!policyGroupsLoading &&
		team?.policy_group_id != null &&
		(currentPolicyGroup === null ||
			!currentPolicyGroup.is_enabled ||
			currentPolicyGroup.items.length === 0);
	const canMutateTeam = team != null && team.archived_at == null;
	const hasChanges =
		canMutateTeam &&
		(name.trim() !== team.name ||
			(description.trim() || "") !== team.description ||
			selectedPolicyGroupId !== (team.policy_group_id ?? null));
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
			ADMIN_TEAM_DETAIL_TAB_INDEX[pageTab] >=
				ADMIN_TEAM_DETAIL_TAB_INDEX[pageLayoutTab]
				? "forward"
				: "backward",
		);
		setPageLayoutTab(pageTab);
	}, [isPageLayout, pageLayoutTab, pageTab]);

	const handleSave = async () => {
		if (!team || !canMutateTeam) {
			return;
		}

		const nextName = name.trim();
		const nextPolicyGroupId = Number(policyGroupId);
		if (!nextName || !Number.isFinite(nextPolicyGroupId)) {
			return;
		}

		try {
			setSaving(true);
			await adminTeamService.update(team.id, {
				name: nextName,
				description: description.trim() || undefined,
				policy_group_id: nextPolicyGroupId,
			});
			await Promise.all([
				loadTeamDetail(team.id),
				loadAuditEntries(team.id),
				onListChange(),
			]);
			toast.success(t("team_updated"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setSaving(false);
		}
	};

	const handleArchive = async () => {
		if (!team || !canMutateTeam) {
			return;
		}

		try {
			setArchiving(true);
			await adminTeamService.delete(team.id);
			await Promise.all([
				loadTeamDetail(team.id),
				loadAuditEntries(team.id),
				onListChange(),
			]);
			setArchiveDialogOpen(false);
			toast.success(t("team_deleted"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setArchiving(false);
		}
	};

	const handleRestore = async () => {
		if (!team || team.archived_at == null) {
			return;
		}

		try {
			setRestoring(true);
			await adminTeamService.restore(team.id);
			await Promise.all([
				loadTeamDetail(team.id),
				loadAuditEntries(team.id),
				onListChange(),
			]);
			toast.success(t("team_restored"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setRestoring(false);
		}
	};

	const handleAddMember = async (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (teamId == null || !canMutateTeam) {
			return;
		}

		const identifier = memberIdentifier.trim();
		if (!identifier) {
			return;
		}

		try {
			setMemberMutating(true);
			await adminTeamService.addMember(teamId, {
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
				onListChange(),
			]);
			toast.success(t("settings:settings_team_member_added"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setMemberMutating(false);
		}
	};

	const handleUpdateMemberRole = async (
		memberUserId: number,
		role: TeamMemberRole,
	) => {
		if (teamId == null || !canMutateTeam) {
			return;
		}

		try {
			setMemberMutating(true);
			await adminTeamService.updateMember(teamId, memberUserId, { role });
			await Promise.all([
				loadTeamDetail(teamId),
				loadMembers(teamId, memberOffset),
				loadAuditEntries(teamId),
				onListChange(),
			]);
			toast.success(t("settings:settings_team_member_role_updated"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setMemberMutating(false);
		}
	};

	const handleRemoveMember = async (memberUserId: number) => {
		if (teamId == null || !canMutateTeam) {
			return;
		}

		try {
			setMemberMutating(true);
			await adminTeamService.removeMember(teamId, memberUserId);
			await Promise.all([
				loadTeamDetail(teamId),
				loadMembers(teamId, memberOffset),
				loadAuditEntries(teamId),
				onListChange(),
			]);
			toast.success(t("settings:settings_team_member_removed"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setMemberMutating(false);
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
				<DialogContent className="flex max-h-[min(860px,calc(100vh-2rem))] flex-col gap-0 overflow-hidden p-0 sm:max-w-[min(1180px,calc(100vw-2rem))]">
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
					ADMIN_TEAM_DETAIL_TAB_INDEX[value] >=
						ADMIN_TEAM_DETAIL_TAB_INDEX[currentTab]
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
			<div className="mb-5 flex items-start justify-between gap-3">
				<div>
					<h4 className="text-base font-semibold text-foreground">
						{t("edit_team")}
					</h4>
					<p className="mt-1 text-sm text-muted-foreground">
						{t("team_details_desc")}
					</p>
				</div>
				<Button
					type="button"
					variant="ghost"
					size="sm"
					className={ADMIN_CONTROL_HEIGHT_CLASS}
					onClick={() => void onRefreshPolicyGroups()}
					disabled={policyGroupsLoading}
				>
					<Icon
						name={policyGroupsLoading ? "Spinner" : "ArrowsClockwise"}
						className={`mr-1 h-3.5 w-3.5 ${policyGroupsLoading ? "animate-spin" : ""}`}
					/>
					{t("refresh")}
				</Button>
			</div>
			{detailLoading && !team ? (
				<SkeletonTable columns={2} rows={4} />
			) : (
				<form
					className="space-y-4"
					onSubmit={(event) => {
						event.preventDefault();
						void handleSave();
					}}
				>
					<div className="grid gap-5 md:grid-cols-2">
						<div className="space-y-2 md:col-span-2">
							<Label htmlFor="admin-team-detail-name">{t("core:name")}</Label>
							<Input
								id="admin-team-detail-name"
								value={name}
								maxLength={128}
								disabled={
									detailLoading ||
									saving ||
									archiving ||
									restoring ||
									!canMutateTeam
								}
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onChange={(event) => setName(event.target.value)}
							/>
						</div>
						<div className="space-y-2 md:col-span-2">
							<Label>{t("team_policy_group")}</Label>
							<Select
								items={policyGroupOptions}
								value={policyGroupId}
								onValueChange={(value) => setPolicyGroupId(value ?? "")}
							>
								<SelectTrigger
									disabled={
										detailLoading ||
										saving ||
										archiving ||
										restoring ||
										policyGroupsLoading ||
										!canMutateTeam
									}
								>
									<SelectValue placeholder={t("select_policy_group")} />
								</SelectTrigger>
								<SelectContent>
									{policyGroupOptions.map((option) => (
										<SelectItem
											key={option.value}
											value={option.value}
											disabled={option.disabled}
										>
											{option.label}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
							<p className="text-xs text-muted-foreground">
								{t("team_policy_group_desc")}
							</p>
							{assignedPolicyGroupIsInvalid ? (
								<p className="text-xs text-destructive">
									{t("policy_group_invalid_assignment")}
								</p>
							) : null}
							{policyGroupUnavailable ? (
								<p className="text-xs text-destructive">
									{t("policy_group_no_assignable_groups")}
								</p>
							) : null}
						</div>
						<div className="space-y-2 md:col-span-2">
							<Label htmlFor="admin-team-detail-description">
								{t("description")}
							</Label>
							<textarea
								id="admin-team-detail-description"
								value={description}
								disabled={
									detailLoading ||
									saving ||
									archiving ||
									restoring ||
									!canMutateTeam
								}
								rows={6}
								className="min-h-32 w-full rounded-lg border border-input bg-transparent px-3 py-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:bg-input/50"
								onChange={(event) => setDescription(event.target.value)}
							/>
						</div>
					</div>
					<div className="flex items-center justify-end gap-2 border-t pt-4">
						<Button
							type="submit"
							disabled={
								detailLoading ||
								saving ||
								!canMutateTeam ||
								!name.trim() ||
								!policyGroupId ||
								!hasChanges
							}
						>
							{saving ? (
								<Icon name="Spinner" className="mr-1 h-4 w-4 animate-spin" />
							) : (
								<Icon name="FloppyDisk" className="mr-1 h-4 w-4" />
							)}
							{t("save_changes")}
						</Button>
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
						placeholder={t("team_member_search_placeholder")}
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
						{t("member_filtered_count", {
							filtered: memberTotal,
							total: team?.member_count ?? memberTotal,
						})}
					</span>
					<span>
						{t("team_owner_count")}: {ownerCount}
					</span>
					<span>
						{t("team_manager_count")}: {managerCount}
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
						{t("clear_filters")}
					</Button>
				) : null}
			</div>

			{canMutateTeam ? (
				<form
					className="mb-4 grid gap-3 rounded-xl border bg-muted/20 p-4 md:grid-cols-[minmax(0,1fr)_180px_auto]"
					onSubmit={(event) => void handleAddMember(event)}
				>
					<div className="space-y-2">
						<Label htmlFor="admin-team-member-identifier">
							{t("settings:settings_team_member_identifier")}
						</Label>
						<Input
							id="admin-team-member-identifier"
							value={memberIdentifier}
							disabled={memberMutating}
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
							disabled={memberMutating || !memberIdentifier.trim()}
						>
							{t("settings:settings_team_add_member")}
						</Button>
					</div>
				</form>
			) : (
				<div className="mb-4 rounded-xl border border-dashed bg-muted/10 px-4 py-3 text-sm text-muted-foreground">
					{t("team_members_readonly_archived")}
				</div>
			)}

			{memberLoading && members.length === 0 ? (
				<SkeletonTable columns={6} rows={5} />
			) : memberTotal === 0 ? (
				<EmptyState
					icon={<Icon name="ListBullets" className="h-10 w-10" />}
					title={
						hasMemberFilters
							? t("team_member_filtered_empty")
							: t("settings:settings_team_no_members")
					}
					description={
						hasMemberFilters
							? t("team_member_filtered_empty_desc")
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
									const canEditRole = canMutateTeam && !memberMutating;
									const canRemove = canMutateTeam && !memberMutating;

									return (
										<TableRow key={member.id}>
											<TableCell>
												<div className="space-y-1">
													<div className="flex items-center gap-2">
														<span className="font-medium">
															{member.username}
														</span>
														<Badge
															className={cn(
																"border",
																getTeamRoleBadgeClass(member.role),
															)}
														>
															{roleLabel(member.role)}
														</Badge>
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
														disabled={memberMutating}
														onClick={() => requestRemoveConfirm(member.user_id)}
													>
														{t("settings:settings_team_remove_member")}
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
								{t("entries_page", {
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

	const auditSection = (
		<section className="rounded-2xl border bg-background/60 p-6">
			<div className="mb-5">
				<h4 className="text-base font-semibold text-foreground">
					{t("team_audit_title")}
				</h4>
				<p className="mt-1 text-sm text-muted-foreground">
					{t("team_audit_desc")}
				</p>
			</div>
			{auditLoading && auditEntries.length === 0 ? (
				<SkeletonTable columns={4} rows={4} />
			) : auditTotal === 0 ? (
				<EmptyState
					icon={<Icon name="Scroll" className="h-10 w-10" />}
					title={t("team_audit_empty")}
					description={t("team_audit_empty_desc")}
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
													{t(entry.action, {
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
								{t("entries_page", {
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
	);

	const dangerSection = (
		<section className="rounded-2xl border border-destructive/30 bg-destructive/5 p-6">
			<div className="mb-5">
				<h4 className="text-base font-semibold text-foreground">
					{t("team_danger_zone")}
				</h4>
				<p className="mt-1 text-sm text-muted-foreground">
					{t("team_danger_zone_desc")}
				</p>
			</div>
			<div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
				<div className="space-y-3 rounded-xl border bg-background/70 p-4">
					<div className="flex items-center justify-between gap-3">
						<span className="text-sm text-muted-foreground">
							{t("team_owner_count")}
						</span>
						<span className="font-medium">{ownerCount}</span>
					</div>
					<div className="flex items-center justify-between gap-3">
						<span className="text-sm text-muted-foreground">
							{t("team_manager_count")}
						</span>
						<span className="font-medium">{managerCount}</span>
					</div>
					<div className="flex items-center justify-between gap-3">
						<span className="text-sm text-muted-foreground">
							{t("team_status")}
						</span>
						<span className="font-medium">
							{team?.archived_at ? t("archived_badge") : t("core:active")}
						</span>
					</div>
					<p className="text-xs text-muted-foreground">
						{t("team_danger_zone_hint")}
					</p>
				</div>
				<div className="space-y-3 rounded-xl border border-destructive/30 bg-background/70 p-4">
					{team?.archived_at ? (
						<>
							<p className="text-sm text-muted-foreground">
								{t("team_restore_danger_desc")}
							</p>
							<Button
								type="button"
								variant="outline"
								disabled={detailLoading || restoring}
								onClick={() => void handleRestore()}
							>
								{restoring ? (
									<Icon name="Spinner" className="mr-1 h-4 w-4 animate-spin" />
								) : (
									<Icon name="ArrowCounterClockwise" className="mr-1 h-4 w-4" />
								)}
								{t("restore")}
							</Button>
						</>
					) : (
						<>
							<div className="space-y-2">
								<Label htmlFor="admin-team-archive-confirm">
									{t("team_archive_confirm_label")}
								</Label>
								<Input
									id="admin-team-archive-confirm"
									value={archiveConfirmValue}
									placeholder={t("team_archive_confirm_placeholder")}
									onChange={(event) =>
										setArchiveConfirmValue(event.target.value)
									}
									className={ADMIN_CONTROL_HEIGHT_CLASS}
								/>
								<p className="text-xs text-muted-foreground">
									{t("team_archive_confirm_hint", {
										name: team?.name ?? "",
									})}
								</p>
							</div>
							<Button
								type="button"
								variant="destructive"
								disabled={
									detailLoading ||
									archiving ||
									archiveConfirmValue.trim() !== (team?.name ?? "")
								}
								onClick={() => setArchiveDialogOpen(true)}
							>
								{archiving ? (
									<Icon name="Spinner" className="mr-1 h-4 w-4 animate-spin" />
								) : (
									<Icon name="Trash" className="mr-1 h-4 w-4" />
								)}
								{t("delete_team")}
							</Button>
						</>
					)}
				</div>
			</div>
		</section>
	);

	return (
		<>
			<Wrapper>
				{isPageLayout ? (
					<div className="flex flex-wrap items-start justify-between gap-3 border-b px-6 pt-5 pb-4">
						<div className="space-y-1">
							<p className="text-xs uppercase tracking-wide text-muted-foreground">
								{t("teams")}
							</p>
							<h1 className="text-xl font-semibold tracking-tight">
								{team?.name ?? t("team_details_title")}
							</h1>
							<p className="text-sm text-muted-foreground">
								{t("team_details_title")}
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
							{t("team_details_title")}
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

						adminTeamDetailContentScrollPositions.set(
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

								adminTeamDetailSidebarScrollPositions.set(
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
												{team?.name ?? t("core:loading")}
											</h3>
											<p className="text-sm text-muted-foreground max-lg:line-clamp-2">
												{team?.description || t("team_no_description")}
											</p>
										</div>
										<div className="flex flex-wrap gap-2">
											{team?.archived_at ? (
												<Badge variant="outline">{t("archived_badge")}</Badge>
											) : (
												<Badge variant="outline">{t("core:active")}</Badge>
											)}
											{team?.policy_group_id != null ? (
												<Badge variant="outline">
													{selectedPolicyGroup?.name ??
														currentPolicyGroup?.name ??
														`PG ${team.policy_group_id}`}
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
											{team?.id ?? "-"}
										</p>
									</div>
									<div className="space-y-1">
										<p className="text-xs uppercase tracking-wide text-muted-foreground">
											{t("created_by")}
										</p>
										<p className="text-sm text-foreground">
											{team
												? `${team.created_by_username} (#${team.created_by})`
												: "-"}
										</p>
									</div>
									<div className="space-y-1">
										<p className="text-xs uppercase tracking-wide text-muted-foreground">
											{t("core:created_at")}
										</p>
										<p className="text-sm text-foreground">
											{team ? formatDateAbsolute(team.created_at) : "-"}
										</p>
									</div>
									<div className="space-y-1">
										<p className="text-xs uppercase tracking-wide text-muted-foreground">
											{t("core:updated_at")}
										</p>
										<p className="text-sm text-foreground">
											{team ? formatDateAbsolute(team.updated_at) : "-"}
										</p>
									</div>
									{team?.archived_at ? (
										<div className="space-y-1">
											<p className="text-xs uppercase tracking-wide text-muted-foreground">
												{t("team_archived_at")}
											</p>
											<p className="text-sm text-foreground">
												{formatDateAbsolute(team.archived_at)}
											</p>
										</div>
									) : null}
								</div>
								<div className="space-y-3 rounded-xl border bg-background/60 p-4 max-lg:p-3">
									<div>
										<p className="text-sm font-medium text-foreground">
											{t("storage")}
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
											<span>{t("member_count")}</span>
											<span>{team?.member_count ?? "-"}</span>
										</div>
										<div className="flex items-center justify-between gap-3">
											<span>{t("team_owner_count")}</span>
											<span>{ownerCount}</span>
										</div>
										<div className="flex items-center justify-between gap-3">
											<span>{t("team_manager_count")}</span>
											<span>{managerCount}</span>
										</div>
									</div>
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
												{t("overview")}
											</TabsTrigger>
											<TabsTrigger
												value="members"
												className="h-10 min-w-0 rounded-none px-0"
											>
												{t("settings:settings_team_members")}
											</TabsTrigger>
											<TabsTrigger
												value="audit"
												className="h-10 min-w-0 rounded-none px-0"
											>
												{t("team_audit_title")}
											</TabsTrigger>
											<TabsTrigger
												value="danger"
												className="h-10 min-w-0 rounded-none px-0"
											>
												{t("team_danger_zone")}
											</TabsTrigger>
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
										<TabsContent
											value="audit"
											className={cn(
												"outline-none",
												currentTab === "audit" && panelAnimationClass,
											)}
										>
											{auditSection}
										</TabsContent>
										<TabsContent
											value="danger"
											className={cn(
												"outline-none",
												currentTab === "danger" && panelAnimationClass,
											)}
										>
											{dangerSection}
										</TabsContent>
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
			</Wrapper>

			<ConfirmDialog
				{...removeDialogProps}
				title={t("settings:settings_team_remove_member")}
				description={
					removeMember
						? `${t("settings:settings_team_remove_member_desc")} @${removeMember.username}`
						: t("settings:settings_team_remove_member_desc")
				}
				confirmLabel={t("settings:settings_team_remove_member")}
				variant="destructive"
			/>

			<ConfirmDialog
				open={archiveDialogOpen}
				onOpenChange={setArchiveDialogOpen}
				title={team ? `${t("delete_team")} "${team.name}"?` : t("delete_team")}
				description={t("archive_team_desc")}
				confirmLabel={t("core:delete")}
				onConfirm={() => void handleArchive()}
				variant="destructive"
			/>
		</>
	);
}
