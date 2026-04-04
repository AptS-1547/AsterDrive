import {
	type FormEvent,
	useEffect,
	useEffectEvent,
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
import { ScrollArea } from "@/components/ui/scroll-area";
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
import { handleApiError } from "@/hooks/useApiError";
import { useConfirmDialog } from "@/hooks/useConfirmDialog";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import { formatBytes, formatDateAbsolute, formatDateShort } from "@/lib/format";
import { getTeamRoleBadgeClass } from "@/lib/team";
import { cn } from "@/lib/utils";
import { adminTeamService } from "@/services/adminService";
import type {
	AdminTeamInfo,
	StoragePolicyGroup,
	TeamMemberInfo,
	TeamMemberRole,
} from "@/types/api";

interface AdminTeamDetailDialogProps {
	open: boolean;
	teamId: number | null;
	policyGroups: StoragePolicyGroup[];
	policyGroupsLoading: boolean;
	onListChange: () => Promise<void>;
	onOpenChange: (open: boolean) => void;
	onRefreshPolicyGroups: () => Promise<void>;
}

interface PolicyGroupOption {
	disabled?: boolean;
	label: string;
	value: string;
}

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
	open,
	teamId,
	policyGroups,
	policyGroupsLoading,
	onListChange,
	onOpenChange,
	onRefreshPolicyGroups,
}: AdminTeamDetailDialogProps) {
	const { t } = useTranslation(["admin", "core", "settings"]);
	const [archiveDialogOpen, setArchiveDialogOpen] = useState(false);
	const [archiving, setArchiving] = useState(false);
	const [detailLoading, setDetailLoading] = useState(false);
	const [memberIdentifier, setMemberIdentifier] = useState("");
	const [memberMutating, setMemberMutating] = useState(false);
	const [memberRole, setMemberRole] = useState<TeamMemberRole>("member");
	const [members, setMembers] = useState<TeamMemberInfo[]>([]);
	const [name, setName] = useState("");
	const [description, setDescription] = useState("");
	const [policyGroupId, setPolicyGroupId] = useState("");
	const [restoring, setRestoring] = useState(false);
	const [saving, setSaving] = useState(false);
	const [team, setTeam] = useState<AdminTeamInfo | null>(null);
	const requestIdRef = useRef(0);
	const roleOptions: TeamMemberRole[] = ["owner", "admin", "member"];

	const roleLabel = (role: TeamMemberRole) =>
		t(`settings:settings_team_role_${role}`);

	const loadTeamData = useEffectEvent(async (nextTeamId: number) => {
		const requestId = ++requestIdRef.current;
		setDetailLoading(true);
		try {
			const [detail, nextMembers] = await Promise.all([
				adminTeamService.get(nextTeamId),
				adminTeamService.listMembers(nextTeamId),
			]);
			if (requestId !== requestIdRef.current) {
				return;
			}
			setTeam(detail);
			setMembers(nextMembers);
		} catch (error) {
			if (requestId !== requestIdRef.current) {
				return;
			}
			setTeam(null);
			setMembers([]);
			handleApiError(error);
		} finally {
			if (requestId === requestIdRef.current) {
				setDetailLoading(false);
			}
		}
	});

	useEffect(() => {
		if (!open || teamId == null) {
			requestIdRef.current += 1;
			setArchiveDialogOpen(false);
			setArchiving(false);
			setDescription("");
			setDetailLoading(false);
			setMemberIdentifier("");
			setMemberMutating(false);
			setMembers([]);
			setMemberRole("member");
			setName("");
			setPolicyGroupId("");
			setRestoring(false);
			setSaving(false);
			setTeam(null);
			return;
		}

		void loadTeamData(teamId);
	}, [open, teamId]);

	useEffect(() => {
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
			await Promise.all([loadTeamData(team.id), onListChange()]);
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
			await Promise.all([loadTeamData(team.id), onListChange()]);
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
			await Promise.all([loadTeamData(team.id), onListChange()]);
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
			await Promise.all([loadTeamData(teamId), onListChange()]);
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
			await Promise.all([loadTeamData(teamId), onListChange()]);
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
			await Promise.all([loadTeamData(teamId), onListChange()]);
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

	return (
		<>
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
					<DialogHeader className="flex items-center justify-center px-6 pt-5 pb-0 text-center">
						<DialogTitle className="text-lg">
							{t("team_details_title")}
						</DialogTitle>
					</DialogHeader>
					<div className="grid min-h-0 flex-1 gap-0 lg:grid-cols-[320px_minmax(0,1fr)]">
						<aside className="space-y-5 border-b bg-muted/20 p-6 lg:sticky lg:top-0 lg:self-start lg:border-r lg:border-b-0">
							<div className="space-y-3">
								<div className="flex size-16 items-center justify-center rounded-2xl bg-primary/10 text-primary">
									<Icon name="Cloud" className="h-7 w-7" />
								</div>
								<div className="space-y-1">
									<h3 className="text-lg font-semibold text-foreground">
										{team?.name ?? t("core:loading")}
									</h3>
									<p className="text-sm text-muted-foreground">
										{team?.description || t("team_no_description")}
									</p>
								</div>
								<div className="flex flex-wrap gap-2">
									{team?.archived_at ? (
										<Badge variant="outline">{t("archived_badge")}</Badge>
									) : null}
									{team?.policy_group_id != null ? (
										<Badge variant="outline">
											{selectedPolicyGroup?.name ??
												currentPolicyGroup?.name ??
												`PG ${team.policy_group_id}`}
										</Badge>
									) : null}
								</div>
							</div>

							<div className="space-y-3 rounded-xl border bg-background/60 p-4">
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

							<div className="space-y-3 rounded-xl border bg-background/60 p-4">
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
								<div className="flex items-center justify-between gap-3 text-xs text-muted-foreground">
									<span>{t("member_count")}</span>
									<span>{team?.member_count ?? "-"}</span>
								</div>
							</div>
						</aside>

						<ScrollArea className="min-h-0">
							<div className="space-y-4 p-6">
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
												name={
													policyGroupsLoading ? "Spinner" : "ArrowsClockwise"
												}
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
													<Label htmlFor="admin-team-detail-name">
														{t("core:name")}
													</Label>
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
														onValueChange={(value) =>
															setPolicyGroupId(value ?? "")
														}
													>
														<SelectTrigger
															className={`${ADMIN_CONTROL_HEIGHT_CLASS} w-full`}
															disabled={
																detailLoading ||
																saving ||
																archiving ||
																restoring ||
																policyGroupsLoading ||
																!canMutateTeam
															}
														>
															<SelectValue
																placeholder={t("select_policy_group")}
															/>
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
														onChange={(event) =>
															setDescription(event.target.value)
														}
													/>
												</div>
											</div>
											<div className="flex flex-wrap items-center justify-between gap-3 border-t pt-4">
												<p className="text-xs text-muted-foreground">
													{t("team_details_footer_hint")}
												</p>
												<div className="flex flex-wrap gap-2">
													{team?.archived_at ? (
														<Button
															type="button"
															variant="outline"
															disabled={detailLoading || restoring}
															onClick={() => void handleRestore()}
														>
															{restoring ? (
																<Icon
																	name="Spinner"
																	className="mr-1 h-4 w-4 animate-spin"
																/>
															) : (
																<Icon
																	name="ArrowCounterClockwise"
																	className="mr-1 h-4 w-4"
																/>
															)}
															{t("restore")}
														</Button>
													) : (
														<Button
															type="button"
															variant="destructive"
															disabled={detailLoading || archiving}
															onClick={() => setArchiveDialogOpen(true)}
														>
															{archiving ? (
																<Icon
																	name="Spinner"
																	className="mr-1 h-4 w-4 animate-spin"
																/>
															) : (
																<Icon name="Trash" className="mr-1 h-4 w-4" />
															)}
															{t("delete_team")}
														</Button>
													)}
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
															<Icon
																name="Spinner"
																className="mr-1 h-4 w-4 animate-spin"
															/>
														) : (
															<Icon
																name="FloppyDisk"
																className="mr-1 h-4 w-4"
															/>
														)}
														{t("save_changes")}
													</Button>
												</div>
											</div>
										</form>
									)}
								</section>

								{canMutateTeam ? (
									<section className="rounded-2xl border bg-background/60 p-6">
										<div className="mb-5">
											<h4 className="text-base font-semibold text-foreground">
												{t("settings:settings_team_members")}
											</h4>
											<p className="mt-1 text-sm text-muted-foreground">
												{t("settings:settings_team_members_desc")}
											</p>
										</div>
										<form
											className="grid gap-3 md:grid-cols-[minmax(0,1fr)_180px_auto]"
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
													placeholder={t(
														"settings:settings_team_member_placeholder",
													)}
													onChange={(event) =>
														setMemberIdentifier(event.target.value)
													}
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
													onValueChange={(value) =>
														setMemberRole(value as TeamMemberRole)
													}
												>
													<SelectTrigger className="w-full">
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
									</section>
								) : null}

								<section className="rounded-2xl border bg-background/60 p-6">
									<div className="mb-5">
										<h4 className="text-base font-semibold text-foreground">
											{t("settings:settings_team_members")}
										</h4>
										<p className="mt-1 text-sm text-muted-foreground">
											{t("settings:settings_team_members_desc")}
										</p>
									</div>
									{detailLoading && members.length === 0 ? (
										<div className="py-8 text-center text-sm text-muted-foreground">
											{t("core:loading")}
										</div>
									) : members.length === 0 ? (
										<EmptyState
											icon={<Icon name="Cloud" className="h-10 w-10" />}
											title={t("settings:settings_team_no_members")}
											description={t("settings:settings_team_no_members_desc")}
										/>
									) : (
										<div className="overflow-x-auto rounded-xl border">
											<Table>
												<TableHeader>
													<TableRow>
														<TableHead>
															{t("settings:settings_team_member")}
														</TableHead>
														<TableHead>
															{t("settings:settings_team_email")}
														</TableHead>
														<TableHead>
															{t("settings:settings_team_status")}
														</TableHead>
														<TableHead>
															{t("settings:settings_team_role_label")}
														</TableHead>
														<TableHead>{t("core:created_at")}</TableHead>
														<TableHead>{t("core:actions")}</TableHead>
													</TableRow>
												</TableHeader>
												<TableBody>
													{members.map((member) => {
														const canEditRole =
															canMutateTeam && !memberMutating;
														const canRemove = canMutateTeam && !memberMutating;

														return (
															<TableRow key={member.id}>
																<TableCell>
																	<div className="space-y-1">
																		<div className="flex items-center gap-2">
																			<span className="font-medium">
																				{member.username}
																			</span>
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
																			<SelectTrigger className="w-[150px]">
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
																		<Badge
																			className={cn(
																				"border",
																				getTeamRoleBadgeClass(member.role),
																			)}
																		>
																			{roleLabel(member.role)}
																		</Badge>
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
																			onClick={() =>
																				requestRemoveConfirm(member.user_id)
																			}
																		>
																			{t(
																				"settings:settings_team_remove_member",
																			)}
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
									)}
								</section>
							</div>
						</ScrollArea>
					</div>
				</DialogContent>
			</Dialog>

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
