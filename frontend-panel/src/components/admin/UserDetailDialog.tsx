import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { UserAvatarImage } from "@/components/common/UserAvatarImage";
import {
	getRoleBadgeClass,
	getStatusBadgeClass,
} from "@/components/common/UserStatusBadge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogFooter,
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
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { handleApiError } from "@/hooks/useApiError";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import { formatBytes, formatDateAbsolute } from "@/lib/format";
import { getNormalizedDisplayName, getUserDisplayName } from "@/lib/user";
import { passwordSchema } from "@/lib/validation";
import {
	adminPolicyGroupService,
	adminUserService,
} from "@/services/adminService";
import type {
	StoragePolicyGroup,
	UpdateUserRequest,
	UserInfo,
	UserRole,
	UserStatus,
} from "@/types/api";

const POLICY_GROUP_PAGE_SIZE = 100;

interface UserDetailDialogProps {
	user: UserInfo | null;
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onUpdate: (id: number, data: UpdateUserRequest) => Promise<void>;
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

export function UserDetailDialog({
	user,
	open,
	onOpenChange,
	onUpdate,
}: UserDetailDialogProps) {
	const { t } = useTranslation("admin");
	const [confirmPasswordValue, setConfirmPasswordValue] = useState("");
	const [quotaValue, setQuotaValue] = useState("");
	const [draftRole, setDraftRole] = useState<UserRole>("user");
	const [draftStatus, setDraftStatus] = useState<UserStatus>("active");
	const [draftPolicyGroupId, setDraftPolicyGroupId] = useState<number | null>(
		null,
	);
	const [passwordValue, setPasswordValue] = useState("");
	const [passwordErrors, setPasswordErrors] = useState<{
		confirm?: string;
		password?: string;
	}>({});
	const [policyGroups, setPolicyGroups] = useState<StoragePolicyGroup[]>([]);
	const [policyGroupsLoading, setPolicyGroupsLoading] = useState(true);
	const [revokingSessions, setRevokingSessions] = useState(false);
	const [savingPassword, setSavingPassword] = useState(false);
	const [savingProfile, setSavingProfile] = useState(false);

	useEffect(() => {
		if (!user) {
			setConfirmPasswordValue("");
			setDraftPolicyGroupId(null);
			setDraftRole("user");
			setDraftStatus("active");
			setPasswordValue("");
			setPasswordErrors({});
			setPolicyGroups([]);
			setPolicyGroupsLoading(true);
			setQuotaValue("");
			setRevokingSessions(false);
			setSavingPassword(false);
			setSavingProfile(false);
			return;
		}

		setConfirmPasswordValue("");
		setDraftPolicyGroupId(user.policy_group_id ?? null);
		setDraftRole(user.role);
		setDraftStatus(user.status);
		setPasswordValue("");
		setPasswordErrors({});
		setQuotaValue(
			user.storage_quota > 0
				? String(Math.round(user.storage_quota / 1024 / 1024))
				: "0",
		);
		setRevokingSessions(false);
	}, [user]);

	const loadPolicyGroups = useCallback(async () => {
		if (!user) return;
		try {
			setPolicyGroupsLoading(true);
			setPolicyGroups(
				await adminPolicyGroupService.listAll(POLICY_GROUP_PAGE_SIZE),
			);
		} catch (e) {
			handleApiError(e);
		} finally {
			setPolicyGroupsLoading(false);
		}
	}, [user]);

	useEffect(() => {
		if (open && user) {
			void loadPolicyGroups();
		}
	}, [loadPolicyGroups, open, user]);

	if (!user) return null;

	const quota = user.storage_quota ?? 0;
	const used = user.storage_used ?? 0;
	const pct = quota > 0 ? Math.min((used / quota) * 100, 100) : 0;
	const isInitialAdmin = user.id === 1;
	const displayName = getUserDisplayName(user);
	const showUsernameSecondary =
		getNormalizedDisplayName(user.profile.display_name) !== null &&
		displayName !== user.username;
	const currentQuotaMb =
		user.storage_quota && user.storage_quota > 0
			? String(Math.round(user.storage_quota / 1024 / 1024))
			: "0";
	// Admin PATCH supports assigning a group, but not clearing an existing one.
	const hasPolicyGroupChange =
		draftPolicyGroupId != null &&
		draftPolicyGroupId !== (user.policy_group_id ?? null);
	const hasProfileChanges =
		draftRole !== user.role ||
		draftStatus !== user.status ||
		quotaValue !== currentQuotaMb ||
		hasPolicyGroupChange;
	const currentAssignedPolicyGroup =
		user.policy_group_id == null
			? null
			: (policyGroups.find((group) => group.id === user.policy_group_id) ??
				null);
	const selectedPolicyGroup =
		draftPolicyGroupId == null
			? null
			: (policyGroups.find((group) => group.id === draftPolicyGroupId) ?? null);
	const policyGroupOptions = buildPolicyGroupOptions(
		policyGroups,
		draftPolicyGroupId,
	);
	const selectedPolicyGroupOption =
		policyGroupOptions.find(
			(option) => option.value === String(draftPolicyGroupId ?? ""),
		) ?? null;
	const assignedPolicyGroupIsInvalid =
		!policyGroupsLoading &&
		user.policy_group_id != null &&
		(currentAssignedPolicyGroup === null ||
			!currentAssignedPolicyGroup.is_enabled ||
			currentAssignedPolicyGroup.items.length === 0);
	const statusOptions = [
		{ label: t("core:active"), value: "active" },
		{ label: t("core:disabled_status"), value: "disabled" },
	] satisfies ReadonlyArray<{ label: string; value: UserStatus }>;
	const roleOptions = [
		{ label: t("role_admin"), value: "admin" },
		{ label: t("role_user"), value: "user" },
	] satisfies ReadonlyArray<{ label: string; value: UserRole }>;

	const runDialogAction = async (
		setLoading: (loading: boolean) => void,
		action: () => Promise<void>,
		successMessage: string,
	) => {
		try {
			setLoading(true);
			await action();
			toast.success(successMessage);
		} catch (e) {
			handleApiError(e);
		} finally {
			setLoading(false);
		}
	};

	const handleProfileSave = async () => {
		const mb = Number.parseInt(quotaValue, 10);
		const newQuota = Number.isNaN(mb) || mb <= 0 ? 0 : mb * 1024 * 1024;
		const data: UpdateUserRequest = {};

		if (draftRole !== user.role) data.role = draftRole;
		if (draftStatus !== user.status) data.status = draftStatus;
		if (newQuota !== (user.storage_quota ?? 0)) data.storage_quota = newQuota;
		if (hasPolicyGroupChange) {
			data.policy_group_id = draftPolicyGroupId;
		}
		if (Object.keys(data).length === 0) return;

		try {
			setSavingProfile(true);
			await onUpdate(user.id, data);
		} finally {
			setSavingProfile(false);
		}
	};

	const handlePasswordReset = async () => {
		const nextErrors: { confirm?: string; password?: string } = {};
		const passwordResult = passwordSchema.safeParse(passwordValue);
		if (!passwordResult.success) {
			nextErrors.password = passwordResult.error.issues[0]?.message ?? "";
		}
		if (confirmPasswordValue !== passwordValue) {
			nextErrors.confirm = t("password_confirm_mismatch");
		}
		setPasswordErrors(nextErrors);
		if (Object.keys(nextErrors).length > 0) return;

		await runDialogAction(
			setSavingPassword,
			async () => {
				await adminUserService.resetPassword(user.id, {
					password: passwordValue,
				});
				setPasswordValue("");
				setConfirmPasswordValue("");
				setPasswordErrors({});
			},
			t("password_reset_success"),
		);
	};

	const handleSessionRevoke = async () => {
		await runDialogAction(
			setRevokingSessions,
			async () => {
				await adminUserService.revokeSessions(user.id);
			},
			t("revoke_sessions_success"),
		);
	};

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="flex max-h-[min(860px,calc(100vh-2rem))] flex-col gap-0 overflow-hidden p-0 sm:max-w-[min(1100px,calc(100vw-2rem))]">
				<DialogHeader className="flex items-center justify-center px-6 pt-5 pb-0 text-center">
					<DialogTitle className="text-lg">{t("user_details")}</DialogTitle>
				</DialogHeader>
				<div className="grid min-h-0 flex-1 gap-0 lg:grid-cols-[320px_minmax(0,1fr)]">
					<aside className="space-y-5 border-b bg-muted/20 p-6 lg:sticky lg:top-0 lg:self-start lg:border-r lg:border-b-0">
						<div className="space-y-3">
							<UserAvatarImage
								avatar={user.profile.avatar}
								name={displayName}
								size="xl"
								className="aspect-square w-full max-w-[220px]"
							/>
							<div className="space-y-1">
								<h3 className="text-lg font-semibold text-foreground">
									{displayName}
								</h3>
								{showUsernameSecondary ? (
									<p className="text-sm text-muted-foreground">
										@{user.username}
									</p>
								) : null}
								<p className="text-sm text-muted-foreground">{user.email}</p>
							</div>
							<div className="flex flex-wrap gap-2">
								<Badge
									variant="outline"
									className={getRoleBadgeClass(user.role)}
								>
									{user.role === "admin" ? "Admin" : "User"}
								</Badge>
								<Badge
									variant="outline"
									className={getStatusBadgeClass(user.status)}
								>
									{user.status === "active"
										? t("core:active")
										: t("core:disabled_status")}
								</Badge>
							</div>
						</div>

						<div className="space-y-3 rounded-xl border bg-background/60 p-4">
							<div className="space-y-1">
								<p className="text-xs uppercase tracking-wide text-muted-foreground">
									ID
								</p>
								<p className="font-mono text-sm text-foreground">{user.id}</p>
							</div>
							<div className="space-y-1">
								<p className="text-xs uppercase tracking-wide text-muted-foreground">
									{t("core:created_at")}
								</p>
								<p className="text-sm text-foreground">
									{formatDateAbsolute(user.created_at)}
								</p>
							</div>
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
							{quota > 0 ? <Progress value={pct} className="h-2" /> : null}
						</div>
					</aside>

					<div className="min-h-0 min-w-0 overflow-y-auto p-6">
						<div className="space-y-4">
							<section className="rounded-2xl border bg-background/60 p-6">
								<div className="mb-5">
									<h4 className="text-base font-semibold text-foreground">
										{t("user_details")}
									</h4>
									<p className="mt-1 text-sm text-muted-foreground">
										{t("storage_quota_settings_desc")}
									</p>
								</div>
								<div className="grid gap-5 md:grid-cols-2">
									<div className="space-y-2">
										<Label>{t("email")}</Label>
										<Input
											value={user.email}
											readOnly
											className={ADMIN_CONTROL_HEIGHT_CLASS}
										/>
									</div>
									<div className="space-y-2">
										<Label>{t("username")}</Label>
										<Input
											value={user.username}
											readOnly
											className={ADMIN_CONTROL_HEIGHT_CLASS}
										/>
									</div>
									<div className="space-y-2">
										<div className="flex items-center justify-between gap-2">
											<Label>{t("core:status")}</Label>
											{isInitialAdmin ? (
												<span className="text-xs text-muted-foreground">
													{t("initial_admin_protected")}
												</span>
											) : null}
										</div>
										<TooltipProvider>
											<Tooltip>
												<TooltipTrigger>
													<div>
														<Select
															value={draftStatus}
															onValueChange={(value) =>
																setDraftStatus(value as UserStatus)
															}
															disabled={isInitialAdmin || savingProfile}
														>
															<SelectTrigger
																className={`${ADMIN_CONTROL_HEIGHT_CLASS} w-full`}
															>
																<SelectValue>
																	{t(
																		draftStatus === "active"
																			? "core:active"
																			: "core:disabled_status",
																	)}
																</SelectValue>
															</SelectTrigger>
															<SelectContent>
																{statusOptions.map((option) => (
																	<SelectItem
																		key={option.value}
																		value={option.value}
																	>
																		{option.label}
																	</SelectItem>
																))}
															</SelectContent>
														</Select>
													</div>
												</TooltipTrigger>
												{isInitialAdmin ? (
													<TooltipContent>
														{t("initial_admin_status_blocked")}
													</TooltipContent>
												) : null}
											</Tooltip>
										</TooltipProvider>
									</div>
									<div className="space-y-2">
										<div className="flex items-center justify-between gap-2">
											<Label>{t("role")}</Label>
											{isInitialAdmin ? (
												<span className="text-xs text-muted-foreground">
													{t("initial_admin_protected")}
												</span>
											) : null}
										</div>
										<TooltipProvider>
											<Tooltip>
												<TooltipTrigger>
													<div>
														<Select
															value={draftRole}
															onValueChange={(value) =>
																setDraftRole(value as UserRole)
															}
															disabled={isInitialAdmin || savingProfile}
														>
															<SelectTrigger
																className={`${ADMIN_CONTROL_HEIGHT_CLASS} w-full`}
															>
																<SelectValue>
																	{draftRole === "admin"
																		? t("role_admin")
																		: t("role_user")}
																</SelectValue>
															</SelectTrigger>
															<SelectContent>
																{roleOptions.map((option) => (
																	<SelectItem
																		key={option.value}
																		value={option.value}
																	>
																		{option.label}
																	</SelectItem>
																))}
															</SelectContent>
														</Select>
													</div>
												</TooltipTrigger>
												{isInitialAdmin ? (
													<TooltipContent>
														{t("initial_admin_role_blocked")}
													</TooltipContent>
												) : null}
											</Tooltip>
										</TooltipProvider>
									</div>
									<div className="space-y-2 md:col-span-2">
										<Label htmlFor="user-storage-quota">{t("quota_mb")}</Label>
										<Input
											id="user-storage-quota"
											type="number"
											value={quotaValue}
											onChange={(event) => setQuotaValue(event.target.value)}
											placeholder={`0 = ${t("core:unlimited").toLowerCase()}`}
											className={ADMIN_CONTROL_HEIGHT_CLASS}
											disabled={savingProfile}
										/>
									</div>
								</div>
							</section>

							<section className="rounded-2xl border bg-background/60 p-5">
								<div className="mb-4 flex items-start justify-between gap-3">
									<div>
										<h4 className="text-base font-semibold text-foreground">
											{t("storage_policy_assignments")}
										</h4>
										<p className="mt-1 text-sm text-muted-foreground">
											{t("storage_policy_assignments_desc")}
										</p>
									</div>
									<Button
										variant="ghost"
										size="sm"
										className={ADMIN_CONTROL_HEIGHT_CLASS}
										onClick={() => void loadPolicyGroups()}
										disabled={policyGroupsLoading}
									>
										<Icon
											name={policyGroupsLoading ? "Spinner" : "ArrowsClockwise"}
											className={`mr-1 h-3.5 w-3.5 ${policyGroupsLoading ? "animate-spin" : ""}`}
										/>
										{t("refresh")}
									</Button>
								</div>

								{policyGroupsLoading ? (
									<SkeletonTable columns={2} rows={3} />
								) : (
									<div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_280px]">
										<div className="space-y-3">
											<div className="space-y-2">
												<Label>{t("policy_groups")}</Label>
												<Select
													value={
														draftPolicyGroupId != null
															? String(draftPolicyGroupId)
															: ""
													}
													onValueChange={(value) => {
														if (!value) {
															// The API does not allow unassigning a policy group.
															return;
														}
														setDraftPolicyGroupId(Number(value));
													}}
													disabled={
														savingProfile || policyGroupOptions.length === 0
													}
												>
													<SelectTrigger
														className={`${ADMIN_CONTROL_HEIGHT_CLASS} w-full`}
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
											</div>

											{assignedPolicyGroupIsInvalid ? (
												<div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-700 dark:text-amber-300">
													{t("policy_group_invalid_assignment")}
												</div>
											) : null}

											{policyGroupOptions.length === 0 ? (
												<p className="text-sm text-muted-foreground">
													{t("policy_group_no_assignable_groups")}
												</p>
											) : null}
										</div>

										<div className="rounded-xl border bg-muted/10 p-4">
											<div className="flex flex-wrap items-center gap-2">
												<p className="text-sm font-semibold text-foreground">
													{selectedPolicyGroup?.name ??
														selectedPolicyGroupOption?.label ??
														t("no_policies_assigned")}
												</p>
												{selectedPolicyGroup?.is_default ? (
													<Badge className="border-blue-300 bg-blue-100 text-blue-700 dark:border-blue-700 dark:bg-blue-900 dark:text-blue-300">
														{t("is_default")}
													</Badge>
												) : null}
											</div>
											{selectedPolicyGroup ? (
												<>
													<p className="mt-2 text-xs text-muted-foreground">
														{selectedPolicyGroup.description ||
															t("policy_group_description_empty")}
													</p>
													<p className="mt-3 text-xs font-medium uppercase tracking-wide text-muted-foreground">
														{t("policy_group_rules_count", {
															count: selectedPolicyGroup.items.length,
														})}
													</p>
													<div className="mt-3 space-y-2">
														{selectedPolicyGroup.items
															.slice(0, 3)
															.map((item) => (
																<div
																	key={item.id}
																	className="rounded-lg border bg-background/80 px-3 py-2"
																>
																	<div className="text-sm font-medium text-foreground">
																		{item.policy.name}
																	</div>
																	<div className="mt-1 text-xs text-muted-foreground">
																		{t("policy_group_priority_short", {
																			priority: item.priority,
																		})}
																	</div>
																</div>
															))}
													</div>
												</>
											) : (
												<p className="mt-2 text-sm text-muted-foreground">
													{t("no_policies_assigned")}
												</p>
											)}
										</div>
									</div>
								)}
							</section>

							<section className="rounded-2xl border bg-background/60 p-5">
								<div className="mb-4">
									<h4 className="text-base font-semibold text-foreground">
										{t("security_actions")}
									</h4>
									<p className="mt-1 text-sm text-muted-foreground">
										{t("security_actions_desc")}
									</p>
								</div>

								<div className="space-y-3">
									<div className="rounded-xl border bg-muted/10 p-4">
										<div className="mb-3">
											<h5 className="text-sm font-semibold text-foreground">
												{t("reset_password")}
											</h5>
											<p className="mt-1 text-sm text-muted-foreground">
												{t("reset_password_desc")}
											</p>
										</div>
										<div className="grid gap-4 md:grid-cols-2">
											<div className="space-y-2">
												<Label htmlFor="user-reset-password">
													{t("password")}
												</Label>
												<Input
													id="user-reset-password"
													name="admin-reset-user-password"
													type="password"
													value={passwordValue}
													onChange={(event) => {
														setPasswordValue(event.target.value);
														setPasswordErrors((prev) => ({
															...prev,
															password: undefined,
														}));
													}}
													autoComplete="new-password"
													className={ADMIN_CONTROL_HEIGHT_CLASS}
													disabled={savingPassword}
													aria-invalid={
														passwordErrors.password ? true : undefined
													}
												/>
												<p className="text-xs text-muted-foreground">
													{t("reset_password_hint")}
												</p>
												{passwordErrors.password ? (
													<p className="text-xs text-destructive">
														{passwordErrors.password}
													</p>
												) : null}
											</div>
											<div className="space-y-2">
												<Label htmlFor="user-reset-password-confirm">
													{t("confirm_password")}
												</Label>
												<Input
													id="user-reset-password-confirm"
													name="admin-reset-user-password-confirm"
													type="password"
													value={confirmPasswordValue}
													onChange={(event) => {
														setConfirmPasswordValue(event.target.value);
														setPasswordErrors((prev) => ({
															...prev,
															confirm: undefined,
														}));
													}}
													autoComplete="new-password"
													className={ADMIN_CONTROL_HEIGHT_CLASS}
													disabled={savingPassword}
													aria-invalid={
														passwordErrors.confirm ? true : undefined
													}
												/>
												{passwordErrors.confirm ? (
													<p className="text-xs text-destructive">
														{passwordErrors.confirm}
													</p>
												) : null}
											</div>
										</div>
										<div className="mt-4 flex justify-end">
											<Button
												type="button"
												onClick={() => void handlePasswordReset()}
												disabled={
													savingPassword ||
													passwordValue.length === 0 ||
													confirmPasswordValue.length === 0
												}
											>
												{t("reset_password")}
											</Button>
										</div>
									</div>

									<div className="rounded-xl border border-destructive/20 bg-destructive/5 p-4">
										<div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
											<div className="max-w-2xl">
												<h5 className="text-sm font-semibold text-foreground">
													{t("revoke_sessions")}
												</h5>
												<p className="mt-1 text-sm text-muted-foreground">
													{t("revoke_sessions_desc")}
												</p>
												<p className="mt-2 text-xs text-muted-foreground">
													{t("revoke_sessions_hint")}
												</p>
											</div>
											<Button
												type="button"
												variant="destructive"
												onClick={() => void handleSessionRevoke()}
												disabled={revokingSessions}
												className="md:shrink-0"
											>
												{revokingSessions ? (
													<Icon
														name="Spinner"
														className="mr-1 h-4 w-4 animate-spin"
													/>
												) : (
													<Icon name="SignOut" className="mr-1 h-4 w-4" />
												)}
												{t("revoke_sessions")}
											</Button>
										</div>
									</div>
								</div>
							</section>
						</div>
					</div>
				</div>
				<DialogFooter className="mx-0 mb-0 rounded-b-xl px-6 py-2.5 sm:flex-row sm:items-center sm:justify-end">
					{hasProfileChanges ? (
						<Button
							onClick={() => void handleProfileSave()}
							disabled={savingProfile}
						>
							{savingProfile ? (
								<Icon name="Spinner" className="mr-1 h-4 w-4 animate-spin" />
							) : null}
							{t("save_changes")}
						</Button>
					) : null}
					<Button variant="outline" onClick={() => onOpenChange(false)}>
						{t("core:close")}
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
