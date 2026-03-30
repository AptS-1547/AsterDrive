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
import { Switch } from "@/components/ui/switch";
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
	adminPolicyService,
	adminUserPolicyService,
	adminUserService,
} from "@/services/adminService";
import type {
	StoragePolicy,
	UserInfo,
	UserRole,
	UserStatus,
	UserStoragePolicy,
} from "@/types/api";

const POLICY_PAGE_SIZE = 100;

interface UserDetailDialogProps {
	user: UserInfo | null;
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onUpdate: (
		id: number,
		data: { role?: UserRole; status?: UserStatus; storage_quota?: number },
	) => Promise<void>;
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
	const [passwordValue, setPasswordValue] = useState("");
	const [passwordErrors, setPasswordErrors] = useState<{
		confirm?: string;
		password?: string;
	}>({});
	const [revokingSessions, setRevokingSessions] = useState(false);
	const [savingPassword, setSavingPassword] = useState(false);
	const [savingProfile, setSavingProfile] = useState(false);
	const [assignments, setAssignments] = useState<UserStoragePolicy[]>([]);
	const [policies, setPolicies] = useState<StoragePolicy[]>([]);
	const [policiesLoading, setPoliciesLoading] = useState(true);
	const [addPolicyId, setAddPolicyId] = useState<number | null>(null);
	const [addQuota, setAddQuota] = useState("");
	const [addDefault, setAddDefault] = useState(false);
	const [editingAssignmentId, setEditingAssignmentId] = useState<number | null>(
		null,
	);
	const [editingAssignmentQuota, setEditingAssignmentQuota] = useState("");

	useEffect(() => {
		if (!user) {
			setConfirmPasswordValue("");
			setQuotaValue("");
			setDraftRole("user");
			setDraftStatus("active");
			setPasswordValue("");
			setPasswordErrors({});
			setRevokingSessions(false);
			setSavingPassword(false);
			setSavingProfile(false);
			setAssignments([]);
			setPolicies([]);
			setPoliciesLoading(true);
			setAddPolicyId(null);
			setAddQuota("");
			setAddDefault(false);
			setEditingAssignmentId(null);
			setEditingAssignmentQuota("");
			return;
		}
		setQuotaValue(
			user.storage_quota > 0
				? String(Math.round(user.storage_quota / 1024 / 1024))
				: "0",
		);
		setConfirmPasswordValue("");
		setDraftRole(user.role);
		setDraftStatus(user.status);
		setPasswordValue("");
		setPasswordErrors({});
		setRevokingSessions(false);
	}, [user]);

	const loadPolicies = useCallback(async () => {
		if (!user) return;
		try {
			setPoliciesLoading(true);
			const [a, p] = await Promise.all([
				adminUserPolicyService.list(user.id, {
					limit: POLICY_PAGE_SIZE,
					offset: 0,
				}),
				adminPolicyService.list({ limit: POLICY_PAGE_SIZE, offset: 0 }),
			]);
			setAssignments(a.items);
			setPolicies(p.items);
		} catch (e) {
			handleApiError(e);
		} finally {
			setPoliciesLoading(false);
		}
	}, [user]);

	useEffect(() => {
		if (open && user) {
			void loadPolicies();
		}
	}, [open, user, loadPolicies]);

	if (!user) return null;

	const quota = user.storage_quota ?? 0;
	const used = user.storage_used ?? 0;
	const pct = quota > 0 ? Math.min((used / quota) * 100, 100) : 0;
	const isInitialAdmin = user.id === 1;
	const displayName = getUserDisplayName(user);
	const showUsernameSecondary =
		getNormalizedDisplayName(user.profile.display_name) !== null &&
		displayName !== user.username;

	const handleProfileSave = async () => {
		const mb = Number.parseInt(quotaValue, 10);
		const newQuota = Number.isNaN(mb) || mb <= 0 ? 0 : mb * 1024 * 1024;
		const data: {
			role?: UserRole;
			status?: UserStatus;
			storage_quota?: number;
		} = {};
		if (draftRole !== user.role) data.role = draftRole;
		if (draftStatus !== user.status) data.status = draftStatus;
		if (newQuota !== (user.storage_quota ?? 0)) data.storage_quota = newQuota;
		if (Object.keys(data).length === 0) return;
		try {
			setSavingProfile(true);
			await onUpdate(user.id, data);
		} finally {
			setSavingProfile(false);
		}
	};

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

	const policyName = (policyId: number) =>
		policies.find((p) => p.id === policyId)?.name ?? `#${policyId}`;

	const handleAssign = async () => {
		if (!addPolicyId) return;
		try {
			const mb = Number.parseInt(addQuota, 10);
			const quotaBytes = Number.isNaN(mb) || mb <= 0 ? 0 : mb * 1024 * 1024;
			await adminUserPolicyService.assign(user.id, {
				policy_id: addPolicyId,
				is_default: addDefault,
				quota_bytes: quotaBytes,
			});
			setAddPolicyId(null);
			setAddQuota("");
			setAddDefault(false);
			toast.success(t("policy_assigned"));
			await loadPolicies();
		} catch (e) {
			handleApiError(e);
		}
	};

	const handleSetDefault = async (a: UserStoragePolicy) => {
		if (a.is_default) return;
		try {
			await adminUserPolicyService.update(user.id, a.id, {
				is_default: true,
			});
			await loadPolicies();
		} catch (e) {
			handleApiError(e);
		}
	};

	const handleRemove = async (assignment: UserStoragePolicy) => {
		if (assignment.is_default && assignments.length > 1) {
			toast.error(t("default_policy_remove_blocked"));
			return;
		}
		try {
			await adminUserPolicyService.remove(user.id, assignment.id);
			toast.success(t("assignment_removed"));
			await loadPolicies();
		} catch (e) {
			handleApiError(e);
		}
	};

	const startEditAssignmentQuota = (assignment: UserStoragePolicy) => {
		setEditingAssignmentId(assignment.id);
		setEditingAssignmentQuota(
			assignment.quota_bytes > 0
				? String(Math.round(assignment.quota_bytes / 1024 / 1024))
				: "0",
		);
	};

	const saveAssignmentQuota = async (assignment: UserStoragePolicy) => {
		try {
			const mb = Number.parseInt(editingAssignmentQuota, 10);
			const quotaBytes = Number.isNaN(mb) || mb <= 0 ? 0 : mb * 1024 * 1024;
			await adminUserPolicyService.update(user.id, assignment.id, {
				quota_bytes: quotaBytes,
			});
			setEditingAssignmentId(null);
			setEditingAssignmentQuota("");
			toast.success(t("assignment_updated"));
			await loadPolicies();
		} catch (e) {
			handleApiError(e);
		}
	};

	const availablePolicies = policies.filter(
		(p) => !assignments.some((a) => a.policy_id === p.id),
	);
	const hasDefaultPolicy = assignments.some((a) => a.is_default);
	const currentQuotaMb =
		user.storage_quota && user.storage_quota > 0
			? String(Math.round(user.storage_quota / 1024 / 1024))
			: "0";
	const hasProfileChanges =
		draftRole !== user.role ||
		draftStatus !== user.status ||
		quotaValue !== currentQuotaMb;

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="flex max-h-[min(880px,calc(100vh-2rem))] flex-col gap-0 overflow-hidden sm:max-w-[min(1100px,calc(100vw-2rem))] p-0">
				<DialogHeader className="flex items-center justify-center px-6 pt-5 pb-0 text-center">
					<DialogTitle className="text-lg">{t("user_details")}</DialogTitle>
				</DialogHeader>
				<div className="grid min-h-0 flex-1 gap-0 lg:grid-cols-[320px_minmax(0,1fr)]">
					<aside className="space-y-5 border-b bg-muted/20 p-6 lg:border-r lg:border-b-0">
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
						<div className="space-y-5">
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
																<SelectValue />
															</SelectTrigger>
															<SelectContent>
																<SelectItem value="active">
																	{t("core:active")}
																</SelectItem>
																<SelectItem value="disabled">
																	{t("core:disabled_status")}
																</SelectItem>
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
																<SelectValue />
															</SelectTrigger>
															<SelectContent>
																<SelectItem value="admin">
																	{t("role_admin")}
																</SelectItem>
																<SelectItem value="user">
																	{t("role_user")}
																</SelectItem>
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
										<div className="flex gap-3">
											<Input
												id="user-storage-quota"
												type="number"
												value={quotaValue}
												onChange={(e) => setQuotaValue(e.target.value)}
												placeholder={`0 = ${t("core:unlimited").toLowerCase()}`}
												className={ADMIN_CONTROL_HEIGHT_CLASS}
												disabled={savingProfile}
											/>
										</div>
									</div>
								</div>
							</section>

							<section className="rounded-2xl border bg-background/60 p-6">
								<div className="mb-5">
									<h4 className="text-base font-semibold text-foreground">
										{t("reset_password")}
									</h4>
									<p className="mt-1 text-sm text-muted-foreground">
										{t("reset_password_desc")}
									</p>
								</div>
								<div className="grid gap-5 md:grid-cols-2">
									<div className="space-y-2">
										<Label htmlFor="user-reset-password">{t("password")}</Label>
										<Input
											id="user-reset-password"
											type="password"
											value={passwordValue}
											onChange={(e) => {
												setPasswordValue(e.target.value);
												setPasswordErrors((prev) => ({
													...prev,
													password: undefined,
												}));
											}}
											className={ADMIN_CONTROL_HEIGHT_CLASS}
											disabled={savingPassword}
											aria-invalid={passwordErrors.password ? true : undefined}
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
											type="password"
											value={confirmPasswordValue}
											onChange={(e) => {
												setConfirmPasswordValue(e.target.value);
												setPasswordErrors((prev) => ({
													...prev,
													confirm: undefined,
												}));
											}}
											className={ADMIN_CONTROL_HEIGHT_CLASS}
											disabled={savingPassword}
											aria-invalid={passwordErrors.confirm ? true : undefined}
										/>
										{passwordErrors.confirm ? (
											<p className="text-xs text-destructive">
												{passwordErrors.confirm}
											</p>
										) : null}
									</div>
								</div>
								<div className="mt-5 flex justify-end border-t pt-4">
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
							</section>

							<section className="rounded-2xl border bg-background/60 p-6">
								<div className="mb-5">
									<h4 className="text-base font-semibold text-foreground">
										{t("revoke_sessions")}
									</h4>
									<p className="mt-1 text-sm text-muted-foreground">
										{t("revoke_sessions_desc")}
									</p>
								</div>
								<div className="rounded-xl border border-destructive/20 bg-destructive/5 p-4">
									<p className="text-sm text-muted-foreground">
										{t("revoke_sessions_hint")}
									</p>
								</div>
								<div className="mt-5 flex justify-end border-t pt-4">
									<Button
										type="button"
										variant="destructive"
										onClick={() => void handleSessionRevoke()}
										disabled={revokingSessions}
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
							</section>

							<section className="rounded-2xl border bg-background/60 p-6">
								<div className="mb-5 flex items-center justify-between gap-3">
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
										onClick={() => void loadPolicies()}
										disabled={policiesLoading}
									>
										<Icon
											name={policiesLoading ? "Spinner" : "ArrowsClockwise"}
											className={`mr-1 h-3.5 w-3.5 ${policiesLoading ? "animate-spin" : ""}`}
										/>
										{t("refresh")}
									</Button>
								</div>

								{policiesLoading ? (
									<SkeletonTable columns={4} rows={4} />
								) : assignments.length === 0 ? (
									<p className="text-sm text-muted-foreground">
										{t("no_policies_assigned")}
									</p>
								) : (
									<div className="space-y-2">
										{assignments.map((a) => (
											<div
												key={a.id}
												className="flex items-center justify-between gap-3 rounded-lg border bg-muted/20 p-3"
											>
												<div className="min-w-0 flex-1">
													<div className="text-sm font-medium text-foreground">
														{policyName(a.policy_id)}
													</div>
													{editingAssignmentId === a.id ? (
														<div className="mt-2 flex items-center gap-2">
															<Input
																type="number"
																value={editingAssignmentQuota}
																onChange={(e) =>
																	setEditingAssignmentQuota(e.target.value)
																}
																placeholder={`0 = ${t("core:unlimited").toLowerCase()}`}
																className="h-8 max-w-[180px]"
															/>
															<Button
																size="sm"
																onClick={() => void saveAssignmentQuota(a)}
															>
																{t("core:save")}
															</Button>
															<Button
																variant="ghost"
																size="sm"
																onClick={() => {
																	setEditingAssignmentId(null);
																	setEditingAssignmentQuota("");
																}}
															>
																{t("core:cancel")}
															</Button>
														</div>
													) : (
														<div className="text-xs text-muted-foreground">
															{t("quota")}:{" "}
															{a.quota_bytes > 0
																? formatBytes(a.quota_bytes)
																: t("core:unlimited")}
														</div>
													)}
												</div>
												<div className="flex items-center gap-2">
													{a.is_default ? (
														<Badge className="border-blue-300 bg-blue-100 text-blue-700 dark:border-blue-700 dark:bg-blue-900 dark:text-blue-300">
															{t("is_default")}
														</Badge>
													) : null}
													<Button
														variant="ghost"
														size="sm"
														onClick={() => void handleSetDefault(a)}
														disabled={a.is_default}
													>
														{a.is_default
															? t("default_selected")
															: t("set_default")}
													</Button>
													<Button
														variant="ghost"
														size="sm"
														onClick={() => startEditAssignmentQuota(a)}
													>
														{t("edit_quota")}
													</Button>
													<Button
														variant="ghost"
														size="icon"
														className="h-8 w-8 text-destructive"
														onClick={() => void handleRemove(a)}
													>
														<Icon name="Trash" className="h-3.5 w-3.5" />
													</Button>
												</div>
											</div>
										))}
									</div>
								)}

								{availablePolicies.length > 0 ? (
									<div className="mt-5 space-y-4 rounded-xl border bg-muted/10 p-4">
										<Label className="text-sm font-medium">
											{t("assign_policy")}
										</Label>
										{!hasDefaultPolicy ? (
											<p className="text-xs text-amber-600 dark:text-amber-400">
												{t("default_policy_required")}
											</p>
										) : null}
										<div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_220px_auto] md:items-center">
											<Select
												value={addPolicyId != null ? String(addPolicyId) : ""}
												onValueChange={(v) =>
													setAddPolicyId(v ? Number(v) : null)
												}
											>
												<SelectTrigger>
													<SelectValue placeholder={t("select_policy")} />
												</SelectTrigger>
												<SelectContent>
													{availablePolicies.map((p) => (
														<SelectItem key={p.id} value={String(p.id)}>
															{p.name} ({p.driver_type})
														</SelectItem>
													))}
												</SelectContent>
											</Select>
											<Input
												placeholder={`${t("quota")} (MB, 0=${t("core:unlimited").toLowerCase()})`}
												value={addQuota}
												onChange={(e) => setAddQuota(e.target.value)}
											/>
											<div className="flex items-center justify-between gap-3 md:justify-end">
												<div className="flex items-center gap-2">
													<Switch
														checked={addDefault}
														onCheckedChange={setAddDefault}
													/>
													<span className="text-xs">{t("is_default")}</span>
												</div>
												<Button
													disabled={!addPolicyId}
													onClick={() => void handleAssign()}
												>
													{t("core:confirm")}
												</Button>
											</div>
										</div>
									</div>
								) : null}
							</section>
						</div>
					</div>
				</div>
				<DialogFooter className="mx-0 mb-0 rounded-b-xl px-6 py-3 sm:flex-row sm:items-center sm:justify-end">
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
