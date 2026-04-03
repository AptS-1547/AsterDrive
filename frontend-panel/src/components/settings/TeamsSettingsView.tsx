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
import { SettingsSection } from "@/components/common/SettingsScaffold";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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
import { formatBytes, formatDateShort } from "@/lib/format";
import { cn } from "@/lib/utils";
import { teamService } from "@/services/teamService";
import { useAuthStore } from "@/stores/authStore";
import { useTeamStore } from "@/stores/teamStore";
import type { TeamInfo, TeamMemberInfo, TeamMemberRole } from "@/types/api";

function isTeamManager(role: TeamMemberRole | null | undefined) {
	return role === "owner" || role === "admin";
}

function isTeamOwner(role: TeamMemberRole | null | undefined) {
	return role === "owner";
}

function getTeamRoleBadgeClass(role: TeamMemberRole) {
	if (role === "owner") {
		return "border-amber-500/60 bg-amber-500/10 text-amber-700 dark:text-amber-300";
	}
	if (role === "admin") {
		return "border-blue-500/60 bg-blue-500/10 text-blue-700 dark:text-blue-300";
	}
	return "border-border bg-muted/40 text-muted-foreground";
}

export function TeamsSettingsView() {
	const { t } = useTranslation(["core", "settings"]);
	const navigate = useNavigate();
	const user = useAuthStore((state) => state.user);
	const teams = useTeamStore((state) => state.teams);
	const loadingTeams = useTeamStore((state) => state.loading);
	const ensureTeamsLoaded = useTeamStore((state) => state.ensureLoaded);
	const reloadTeams = useTeamStore((state) => state.reload);
	const [archivedTeams, setArchivedTeams] = useState<TeamInfo[]>([]);
	const [archivedLoading, setArchivedLoading] = useState(false);
	const [selectedTeamId, setSelectedTeamId] = useState<number | null>(null);
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
	const selectedTeamSummary =
		selectedTeamId != null
			? (teams.find((team) => team.id === selectedTeamId) ?? null)
			: null;
	const viewerRole =
		teamDetail?.my_role ?? selectedTeamSummary?.my_role ?? null;
	const canManageTeam = isTeamManager(viewerRole);
	const canAssignOwner = isTeamOwner(viewerRole);
	const canArchiveTeam = isTeamOwner(viewerRole);
	const roleOptions: TeamMemberRole[] = canAssignOwner
		? ["owner", "admin", "member"]
		: ["admin", "member"];

	const roleLabel = (role: TeamMemberRole) =>
		t(`settings:settings_team_role_${role}`);

	const loadSelectedTeamData = useEffectEvent(async (teamId: number) => {
		const requestId = ++requestIdRef.current;
		setDetailLoading(true);
		try {
			const [detail, nextMembers] = await Promise.all([
				teamService.get(teamId),
				teamService.listMembers(teamId),
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

	const loadArchivedTeams = useEffectEvent(async () => {
		if (user?.id == null) {
			setArchivedTeams([]);
			return;
		}

		setArchivedLoading(true);
		try {
			const nextArchivedTeams = await teamService.list({ archived: true });
			setArchivedTeams(nextArchivedTeams);
		} catch (error) {
			handleApiError(error);
		} finally {
			setArchivedLoading(false);
		}
	});

	useEffect(() => {
		void ensureTeamsLoaded(user?.id ?? null).catch(() => undefined);
		void loadArchivedTeams();
	}, [ensureTeamsLoaded, user?.id]);

	useEffect(() => {
		if (teams.length === 0) {
			setSelectedTeamId(null);
			return;
		}
		if (
			selectedTeamId == null ||
			!teams.some((team) => team.id === selectedTeamId)
		) {
			setSelectedTeamId(teams[0]?.id ?? null);
		}
	}, [teams, selectedTeamId]);

	useEffect(() => {
		if (selectedTeamId == null) {
			setTeamDetail(null);
			setMembers([]);
			return;
		}
		void loadSelectedTeamData(selectedTeamId);
	}, [selectedTeamId]);

	useEffect(() => {
		setTeamName(teamDetail?.name ?? "");
		setTeamDescription(teamDetail?.description ?? "");
	}, [teamDetail?.description, teamDetail?.name]);

	const handleUpdateTeam = async (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (!teamDetail) {
			return;
		}

		const name = teamName.trim();
		if (!name) {
			return;
		}

		try {
			setMutating(true);
			await teamService.update(teamDetail.id, {
				name,
				description: teamDescription.trim() || undefined,
			});
			await Promise.all([
				loadSelectedTeamData(teamDetail.id),
				reloadTeams(user?.id ?? null),
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
		if (selectedTeamId == null) {
			return;
		}

		const identifier = memberIdentifier.trim();
		if (!identifier) {
			return;
		}

		try {
			setMutating(true);
			await teamService.addMember(selectedTeamId, {
				identifier,
				role: memberRole,
			});
			setMemberIdentifier("");
			setMemberRole("member");
			await Promise.all([
				loadSelectedTeamData(selectedTeamId),
				reloadTeams(user?.id ?? null),
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
		if (selectedTeamId == null) {
			return;
		}

		try {
			setMutating(true);
			await teamService.updateMember(selectedTeamId, memberUserId, { role });
			await loadSelectedTeamData(selectedTeamId);
			toast.success(t("settings:settings_team_member_role_updated"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setMutating(false);
		}
	};

	const handleRemoveMember = async (memberUserId: number) => {
		if (selectedTeamId == null) {
			return;
		}

		const removingSelf = memberUserId === user?.id;

		try {
			setMutating(true);
			await teamService.removeMember(selectedTeamId, memberUserId);
			await reloadTeams(user?.id ?? null);
			if (removingSelf) {
				setSelectedTeamId(null);
				setTeamDetail(null);
				setMembers([]);
				toast.success(t("settings:settings_team_left"));
			} else {
				await loadSelectedTeamData(selectedTeamId);
				toast.success(t("settings:settings_team_member_removed"));
			}
		} catch (error) {
			handleApiError(error);
		} finally {
			setMutating(false);
		}
	};

	const handleArchiveTeam = async () => {
		if (selectedTeamId == null) {
			return;
		}

		try {
			setMutating(true);
			await teamService.delete(selectedTeamId);
			await Promise.all([reloadTeams(user?.id ?? null), loadArchivedTeams()]);
			setArchiveDialogOpen(false);
			setSelectedTeamId(null);
			setTeamDetail(null);
			setMembers([]);
			toast.success(t("settings:settings_team_deleted"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setMutating(false);
		}
	};

	const handleRestoreTeam = async (teamId: number) => {
		try {
			setMutating(true);
			const restored = await teamService.restore(teamId);
			await Promise.all([reloadTeams(user?.id ?? null), loadArchivedTeams()]);
			setSelectedTeamId(restored.id);
			toast.success(t("settings:settings_team_restored"));
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

	return (
		<>
			<SettingsSection
				title={t("settings:settings_teams")}
				description={t("settings:settings_teams_desc")}
				contentClassName="pt-4"
			>
				{loadingTeams && teams.length === 0 ? (
					<div className="py-10 text-center text-sm text-muted-foreground">
						{t("core:loading")}
					</div>
				) : teams.length === 0 ? (
					<EmptyState
						icon={<Icon name="Cloud" className="h-10 w-10" />}
						title={t("settings:settings_teams_empty_title")}
						description={t("settings:settings_teams_empty_desc")}
					/>
				) : (
					<div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
						{teams.map((team) => {
							const selected = team.id === selectedTeamId;
							return (
								<div
									key={team.id}
									className={cn(
										"rounded-xl border p-4 transition-colors",
										selected ? "border-primary/40 bg-primary/5" : "bg-muted/15",
									)}
								>
									<div className="flex items-start justify-between gap-3">
										<div className="space-y-1">
											<p className="font-semibold">{team.name}</p>
											{team.description ? (
												<p className="text-sm text-muted-foreground">
													{team.description}
												</p>
											) : null}
										</div>
										<Badge className={getTeamRoleBadgeClass(team.my_role)}>
											{roleLabel(team.my_role)}
										</Badge>
									</div>
									<div className="mt-4 space-y-2 text-sm text-muted-foreground">
										<div className="flex items-center justify-between gap-3">
											<span>{t("settings:settings_team_members_count")}</span>
											<span>{team.member_count}</span>
										</div>
										<div className="flex items-center justify-between gap-3">
											<span>{t("settings:settings_team_created_by")}</span>
											<span className="truncate">
												{team.created_by_username}
											</span>
										</div>
										<div className="flex items-center justify-between gap-3">
											<span>{t("settings:settings_team_quota")}</span>
											<span>
												{formatBytes(team.storage_used)}
												{team.storage_quota > 0
													? ` / ${formatBytes(team.storage_quota)}`
													: ` / ${t("core:unlimited")}`}
											</span>
										</div>
									</div>
									<div className="mt-4 flex gap-2">
										<Button
											type="button"
											variant={selected ? "default" : "outline"}
											onClick={() => setSelectedTeamId(team.id)}
										>
											{t("core:manage")}
										</Button>
										<Button
											type="button"
											variant="ghost"
											onClick={() =>
												navigate(`/teams/${team.id}`, {
													viewTransition: true,
												})
											}
										>
											{t("settings:settings_team_open_workspace")}
										</Button>
									</div>
								</div>
							);
						})}
					</div>
				)}
			</SettingsSection>

			{archivedLoading || archivedTeams.length > 0 ? (
				<SettingsSection
					title={t("settings:settings_archived_teams")}
					description={t("settings:settings_archived_teams_desc")}
					contentClassName="pt-4"
				>
					{archivedLoading && archivedTeams.length === 0 ? (
						<div className="py-10 text-center text-sm text-muted-foreground">
							{t("core:loading")}
						</div>
					) : (
						<div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
							{archivedTeams.map((team) => (
								<div
									key={team.id}
									className="rounded-xl border border-dashed bg-muted/10 p-4"
								>
									<div className="flex items-start justify-between gap-3">
										<div className="space-y-1">
											<p className="font-semibold">{team.name}</p>
											{team.description ? (
												<p className="text-sm text-muted-foreground">
													{team.description}
												</p>
											) : null}
										</div>
										<div className="flex flex-wrap justify-end gap-2">
											<Badge variant="outline">
												{t("settings:settings_team_archived_badge")}
											</Badge>
											<Badge className={getTeamRoleBadgeClass(team.my_role)}>
												{roleLabel(team.my_role)}
											</Badge>
										</div>
									</div>
									<div className="mt-4 space-y-2 text-sm text-muted-foreground">
										<div className="flex items-center justify-between gap-3">
											<span>{t("settings:settings_team_members_count")}</span>
											<span>{team.member_count}</span>
										</div>
										<div className="flex items-center justify-between gap-3">
											<span>{t("settings:settings_team_created_by")}</span>
											<span className="truncate">
												{team.created_by_username}
											</span>
										</div>
										<div className="flex items-center justify-between gap-3">
											<span>{t("settings:settings_team_archived_at")}</span>
											<span>
												{team.archived_at
													? formatDateShort(team.archived_at)
													: "-"}
											</span>
										</div>
									</div>
									{isTeamManager(team.my_role) ? (
										<div className="mt-4">
											<Button
												type="button"
												variant="outline"
												disabled={mutating}
												onClick={() => void handleRestoreTeam(team.id)}
											>
												{t("settings:settings_team_restore")}
											</Button>
										</div>
									) : null}
								</div>
							))}
						</div>
					)}
				</SettingsSection>
			) : null}

			{selectedTeamSummary ? (
				<SettingsSection
					title={t("settings:settings_team_details")}
					description={t("settings:settings_team_details_desc")}
					contentClassName="pt-4"
				>
					<div className="grid gap-5 rounded-xl border bg-muted/20 p-4 lg:grid-cols-[280px_minmax(0,1fr)]">
						<div className="rounded-xl border bg-background p-4">
							<div className="space-y-4">
								<div className="space-y-1">
									<p className="text-sm font-semibold">
										{teamDetail?.name ?? selectedTeamSummary.name}
									</p>
									<p className="text-sm text-muted-foreground">
										{teamDetail?.description ||
											t("settings:settings_team_no_description")}
									</p>
								</div>
								<div className="space-y-3 border-t pt-4 text-sm">
									<div className="flex items-center justify-between gap-3">
										<span className="text-muted-foreground">
											{t("settings:settings_team_created_by")}
										</span>
										<span>{teamDetail?.created_by_username ?? "-"}</span>
									</div>
									<div className="flex items-center justify-between gap-3">
										<span className="text-muted-foreground">
											{t("settings:settings_team_my_role")}
										</span>
										{viewerRole ? (
											<Badge className={getTeamRoleBadgeClass(viewerRole)}>
												{roleLabel(viewerRole)}
											</Badge>
										) : (
											<span>-</span>
										)}
									</div>
									<div className="flex items-center justify-between gap-3">
										<span className="text-muted-foreground">
											{t("settings:settings_team_members_count")}
										</span>
										<span>
											{teamDetail?.member_count ??
												selectedTeamSummary.member_count}
										</span>
									</div>
									<div className="flex items-center justify-between gap-3">
										<span className="text-muted-foreground">
											{t("settings:settings_team_quota")}
										</span>
										<span>
											{formatBytes(
												teamDetail?.storage_used ??
													selectedTeamSummary.storage_used,
											)}
											{(teamDetail?.storage_quota ??
												selectedTeamSummary.storage_quota) > 0
												? ` / ${formatBytes(
														teamDetail?.storage_quota ??
															selectedTeamSummary.storage_quota,
													)}`
												: ` / ${t("core:unlimited")}`}
										</span>
									</div>
									<div className="flex items-center justify-between gap-3">
										<span className="text-muted-foreground">
											{t("core:created_at")}
										</span>
										<span>
											{teamDetail
												? formatDateShort(teamDetail.created_at)
												: "-"}
										</span>
									</div>
								</div>
							</div>
						</div>

						<form
							className="space-y-4 rounded-xl border bg-background p-4"
							onSubmit={(event) => void handleUpdateTeam(event)}
						>
							<div className="space-y-2">
								<Label htmlFor="team-name">{t("core:name")}</Label>
								<Input
									id="team-name"
									value={teamName}
									maxLength={128}
									readOnly={!canManageTeam}
									disabled={mutating || detailLoading}
									onChange={(event) => setTeamName(event.target.value)}
								/>
							</div>
							<div className="space-y-2">
								<Label htmlFor="team-description">
									{t("settings:settings_team_description")}
								</Label>
								<textarea
									id="team-description"
									value={teamDescription}
									readOnly={!canManageTeam}
									disabled={mutating || detailLoading}
									rows={5}
									className="min-h-28 w-full rounded-lg border border-input bg-transparent px-3 py-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:bg-input/50"
									onChange={(event) => setTeamDescription(event.target.value)}
								/>
							</div>
							<div className="flex flex-wrap items-center justify-between gap-2 border-t pt-4">
								<div className="flex flex-wrap gap-2">
									<Button
										type="button"
										variant="outline"
										onClick={() =>
											navigate(`/teams/${selectedTeamSummary.id}`, {
												viewTransition: true,
											})
										}
									>
										{t("settings:settings_team_open_workspace")}
									</Button>
									{canArchiveTeam ? (
										<Button
											type="button"
											variant="destructive"
											onClick={() => setArchiveDialogOpen(true)}
										>
											{t("settings:settings_team_archive")}
										</Button>
									) : null}
								</div>
								{canManageTeam ? (
									<Button
										type="submit"
										disabled={mutating || detailLoading || !teamName.trim()}
									>
										{t("save")}
									</Button>
								) : null}
							</div>
						</form>
					</div>
				</SettingsSection>
			) : null}

			{selectedTeamSummary ? (
				<SettingsSection
					title={t("settings:settings_team_members")}
					description={t("settings:settings_team_members_desc")}
					contentClassName="space-y-4 pt-4"
				>
					{canManageTeam ? (
						<form
							className="grid gap-3 rounded-xl border bg-muted/20 p-4 md:grid-cols-[minmax(0,1fr)_180px_auto]"
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
					) : null}

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
										const isSelf = member.user_id === user?.id;
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
															onValueChange={(value) =>
																void handleUpdateMemberRole(
																	member.user_id,
																	value as TeamMemberRole,
																)
															}
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
															className={getTeamRoleBadgeClass(member.role)}
														>
															{roleLabel(member.role)}
														</Badge>
													)}
												</TableCell>
												<TableCell className="text-muted-foreground text-sm">
													{formatDateShort(member.created_at)}
												</TableCell>
												<TableCell>
													{canRemove ? (
														<Button
															type="button"
															variant="ghost"
															size="icon"
															className="h-8 w-8 text-destructive"
															onClick={() =>
																requestRemoveConfirm(member.user_id)
															}
															title={
																canRemoveSelf
																	? t("settings:settings_team_leave")
																	: t("settings:settings_team_remove_member")
															}
														>
															<Icon name="Trash" className="h-3.5 w-3.5" />
														</Button>
													) : null}
												</TableCell>
											</TableRow>
										);
									})}
								</TableBody>
							</Table>
						</div>
					)}
				</SettingsSection>
			) : null}

			<ConfirmDialog
				open={archiveDialogOpen}
				onOpenChange={setArchiveDialogOpen}
				title={`${t("settings:settings_team_archive")} "${teamDetail?.name ?? selectedTeamSummary?.name ?? ""}"?`}
				description={t("settings:settings_team_archive_desc")}
				confirmLabel={t("delete")}
				onConfirm={() => void handleArchiveTeam()}
				variant="destructive"
			/>

			<ConfirmDialog
				{...removeDialogProps}
				title={
					removeMember?.user_id === user?.id
						? `${t("settings:settings_team_leave")} "${teamDetail?.name ?? selectedTeamSummary?.name ?? ""}"?`
						: `${t("settings:settings_team_remove_member")} "${removeMember?.username ?? ""}"?`
				}
				description={t("settings:settings_team_remove_member_desc")}
				confirmLabel={
					removeMember?.user_id === user?.id
						? t("settings:settings_team_leave")
						: t("delete")
				}
				variant="destructive"
			/>
		</>
	);
}
