import {
	type FormEvent,
	useEffect,
	useEffectEvent,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { EmptyState } from "@/components/common/EmptyState";
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
import { formatBytes, formatDateAbsolute, formatDateShort } from "@/lib/format";
import { getTeamRoleBadgeClass, isTeamManager, isTeamOwner } from "@/lib/team";
import { cn } from "@/lib/utils";
import { teamService } from "@/services/teamService";
import type { TeamInfo, TeamMemberInfo, TeamMemberRole } from "@/types/api";

interface TeamManageDialogProps {
	currentUserId: number | null;
	onArchivedReload: () => Promise<void>;
	onOpenChange: (open: boolean) => void;
	onTeamsReload: () => Promise<void>;
	open: boolean;
	teamId: number | null;
	teamSummary: TeamInfo | null;
}

export function TeamManageDialog({
	currentUserId,
	onArchivedReload,
	onOpenChange,
	onTeamsReload,
	open,
	teamId,
	teamSummary,
}: TeamManageDialogProps) {
	const { t } = useTranslation(["core", "settings"]);
	const navigate = useNavigate();
	const [teamDetail, setTeamDetail] = useState<TeamInfo | null>(null);
	const [members, setMembers] = useState<TeamMemberInfo[]>([]);
	const [detailLoading, setDetailLoading] = useState(false);
	const [mutating, setMutating] = useState(false);
	const [teamName, setTeamName] = useState("");
	const [teamDescription, setTeamDescription] = useState("");
	const [memberIdentifier, setMemberIdentifier] = useState("");
	const [memberRole, setMemberRole] = useState<TeamMemberRole>("member");
	const [archiveDialogOpen, setArchiveDialogOpen] = useState(false);
	const requestIdRef = useRef(0);
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

	const loadTeamData = useEffectEvent(async (nextTeamId: number) => {
		const requestId = ++requestIdRef.current;
		setDetailLoading(true);
		try {
			const [detail, nextMembers] = await Promise.all([
				teamService.get(nextTeamId),
				teamService.listMembers(nextTeamId),
			]);
			if (requestId !== requestIdRef.current) {
				return;
			}
			setTeamDetail(detail);
			setMembers(nextMembers);
		} catch (error) {
			if (requestId !== requestIdRef.current) {
				return;
			}
			setTeamDetail(null);
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
			setTeamDetail(null);
			setMembers([]);
			setDetailLoading(false);
			setMutating(false);
			setArchiveDialogOpen(false);
			setTeamName("");
			setTeamDescription("");
			setMemberIdentifier("");
			setMemberRole("member");
			return;
		}

		void loadTeamData(teamId);
	}, [open, teamId]);

	useEffect(() => {
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

	const handleUpdateTeam = async (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (!teamDetail) {
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
			await Promise.all([loadTeamData(teamDetail.id), onTeamsReload()]);
			toast.success(t("settings:settings_team_updated"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setMutating(false);
		}
	};

	const handleAddMember = async (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (teamId == null) {
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
			await Promise.all([loadTeamData(teamId), onTeamsReload()]);
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
		if (teamId == null) {
			return;
		}

		try {
			setMutating(true);
			await teamService.updateMember(teamId, memberUserId, { role });
			await loadTeamData(teamId);
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
				await loadTeamData(teamId);
				toast.success(t("settings:settings_team_member_removed"));
			}
		} catch (error) {
			handleApiError(error);
		} finally {
			setMutating(false);
		}
	};

	const handleArchiveTeam = async () => {
		if (teamId == null) {
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
				<DialogContent className="flex max-h-[min(860px,calc(100vh-2rem))] flex-col gap-0 overflow-hidden p-0 sm:max-w-[min(1120px,calc(100vw-2rem))]">
					<DialogHeader className="flex items-center justify-center px-6 pt-5 pb-0 text-center">
						<DialogTitle className="text-lg">
							{t("settings:settings_team_manage_title")}
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
										{teamDetail?.name ?? teamSummary?.name ?? t("core:loading")}
									</h3>
									<p className="text-sm text-muted-foreground">
										{teamDetail?.description ||
											teamSummary?.description ||
											t("settings:settings_team_no_description")}
									</p>
								</div>
								<div className="flex flex-wrap gap-2">
									{viewerRole ? (
										<Badge className={getTeamRoleBadgeClass(viewerRole)}>
											{roleLabel(viewerRole)}
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

							<div className="space-y-3 rounded-xl border bg-background/60 p-4">
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
								<div className="flex items-center justify-between gap-3 text-xs text-muted-foreground">
									<span>{t("settings:settings_team_members_count")}</span>
									<span>
										{teamDetail?.member_count ??
											teamSummary?.member_count ??
											"-"}
									</span>
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
						</aside>

						<ScrollArea className="min-h-0">
							<div className="space-y-4 p-6">
								<section className="rounded-2xl border bg-background/60 p-6">
									<div className="mb-5">
										<h4 className="text-base font-semibold text-foreground">
											{t("settings:settings_team_details")}
										</h4>
										<p className="mt-1 text-sm text-muted-foreground">
											{t("settings:settings_team_details_desc")}
										</p>
									</div>
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
												onChange={(event) =>
													setTeamDescription(event.target.value)
												}
											/>
										</div>
										<div className="flex flex-wrap items-center justify-between gap-3 border-t pt-4">
											<p className="text-xs text-muted-foreground">
												{detailLoading
													? t("core:loading")
													: t("settings:settings_team_dialog_hint")}
											</p>
											<div className="flex flex-wrap gap-2">
												{canArchiveTeam ? (
													<Button
														type="button"
														variant="destructive"
														disabled={mutating || detailLoading}
														onClick={() => setArchiveDialogOpen(true)}
													>
														{t("settings:settings_team_archive")}
													</Button>
												) : null}
												{canManageTeam ? (
													<Button
														type="submit"
														disabled={
															mutating || detailLoading || !teamName.trim()
														}
													>
														{t("save")}
													</Button>
												) : null}
											</div>
										</div>
									</form>
								</section>

								{canManageTeam ? (
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
												<Label htmlFor="team-member-identifier">
													{t("settings:settings_team_member_identifier")}
												</Label>
												<Input
													id="team-member-identifier"
													value={memberIdentifier}
													disabled={mutating}
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
													disabled={mutating || !memberIdentifier.trim()}
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
														const isSelf = member.user_id === currentUserId;
														const canRemoveSelf =
															isSelf && !isTeamOwner(viewerRole);
														const canManageOwner =
															canAssignOwner || member.role !== "owner";
														const canEditRole =
															canManageTeam && canManageOwner && !mutating;
														const canRemove =
															(canManageTeam && canManageOwner) ||
															canRemoveSelf;

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
																			disabled={mutating}
																			onClick={() =>
																				requestRemoveConfirm(member.user_id)
																			}
																		>
																			{isSelf
																				? t("settings:settings_team_leave")
																				: t(
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
				title={
					removeMember?.user_id === currentUserId
						? t("settings:settings_team_leave")
						: t("settings:settings_team_remove_member")
				}
				description={t("settings:settings_team_remove_member_desc")}
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
