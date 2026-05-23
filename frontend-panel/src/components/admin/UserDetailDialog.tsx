import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { handleApiError } from "@/hooks/useApiError";
import { useRetainedDialogValue } from "@/hooks/useRetainedDialogValue";
import {
	loadAdminPolicyGroupLookup,
	readAdminPolicyGroupLookup,
} from "@/lib/adminPolicyGroupLookup";
import { passwordSchema } from "@/lib/validation";
import { adminUserService } from "@/services/adminService";
import type {
	StoragePolicyGroup,
	UpdateUserRequest,
	UserInfo,
	UserRole,
	UserStatus,
} from "@/types/api";
import type { UserPasswordErrors } from "./user-detail-dialog/types";
import { buildPolicyGroupOptions } from "./user-detail-dialog/types";
import { UserDetailSidebar } from "./user-detail-dialog/UserDetailSidebar";
import { UserPolicyGroupSection } from "./user-detail-dialog/UserPolicyGroupSection";
import { UserProfileSection } from "./user-detail-dialog/UserProfileSection";
import { UserSecurityActionsSection } from "./user-detail-dialog/UserSecurityActionsSection";

interface UserDetailDialogProps {
	user: UserInfo | null;
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onUpdate: (id: number, data: UpdateUserRequest) => Promise<void>;
}

export function UserDetailDialog({
	user: inputUser,
	open,
	onOpenChange,
	onUpdate,
}: UserDetailDialogProps) {
	const { t } = useTranslation(["admin", "core"]);
	const { retainedValue: user, handleOpenChangeComplete } =
		useRetainedDialogValue(inputUser, open);
	const initialPolicyGroups = readAdminPolicyGroupLookup();
	const [confirmPasswordValue, setConfirmPasswordValue] = useState("");
	const [draftEmailVerified, setDraftEmailVerified] = useState(false);
	const [quotaValue, setQuotaValue] = useState("");
	const [draftRole, setDraftRole] = useState<UserRole>("user");
	const [draftStatus, setDraftStatus] = useState<UserStatus>("active");
	const [draftPolicyGroupId, setDraftPolicyGroupId] = useState<number | null>(
		null,
	);
	const [passwordValue, setPasswordValue] = useState("");
	const [passwordErrors, setPasswordErrors] = useState<UserPasswordErrors>({});
	const [policyGroups, setPolicyGroups] = useState<StoragePolicyGroup[]>(
		initialPolicyGroups ?? [],
	);
	const [policyGroupsLoading, setPolicyGroupsLoading] = useState(
		initialPolicyGroups == null,
	);
	const [revokingSessions, setRevokingSessions] = useState(false);
	const [savingPassword, setSavingPassword] = useState(false);
	const [savingProfile, setSavingProfile] = useState(false);

	useEffect(() => {
		if (!user) {
			setConfirmPasswordValue("");
			setDraftEmailVerified(false);
			setDraftPolicyGroupId(null);
			setDraftRole("user");
			setDraftStatus("active");
			setPasswordValue("");
			setPasswordErrors({});
			setPolicyGroups(readAdminPolicyGroupLookup() ?? []);
			setPolicyGroupsLoading(readAdminPolicyGroupLookup() == null);
			setQuotaValue("");
			setRevokingSessions(false);
			setSavingPassword(false);
			setSavingProfile(false);
			return;
		}

		setConfirmPasswordValue("");
		setDraftEmailVerified(user.email_verified);
		setDraftPolicyGroupId(user.policy_group_id ?? null);
		setDraftRole(user.role);
		setDraftStatus(user.status);
		setPasswordValue("");
		setPasswordErrors({});
		setQuotaValue(
			user.storage_quota > 0
				? String(Math.round(user.storage_quota / 1024 / 1024))
				: "",
		);
		setRevokingSessions(false);
	}, [user]);

	const loadPolicyGroups = useCallback(async () => {
		if (!user) return;
		try {
			const cachedPolicyGroups = readAdminPolicyGroupLookup();
			if (cachedPolicyGroups != null) {
				setPolicyGroups(cachedPolicyGroups);
				setPolicyGroupsLoading(false);
			} else {
				setPolicyGroupsLoading(true);
			}
			setPolicyGroups(await loadAdminPolicyGroupLookup());
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
	const currentQuotaMb =
		user.storage_quota && user.storage_quota > 0
			? String(Math.round(user.storage_quota / 1024 / 1024))
			: "";
	// Admin PATCH supports assigning a group, but not clearing an existing one.
	const hasPolicyGroupChange =
		draftPolicyGroupId != null &&
		draftPolicyGroupId !== (user.policy_group_id ?? null);
	const hasProfileChanges =
		draftEmailVerified !== user.email_verified ||
		draftRole !== user.role ||
		draftStatus !== user.status ||
		quotaValue !== currentQuotaMb ||
		hasPolicyGroupChange;
	const currentAssignedPolicyGroup =
		user.policy_group_id == null
			? null
			: (policyGroups.find((group) => group.id === user.policy_group_id) ??
				null);
	const policyGroupOptions = buildPolicyGroupOptions(
		policyGroups,
		draftPolicyGroupId,
	);
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
	const emailVerificationOptions = [
		{ label: t("email_verified"), value: "verified" },
		{ label: t("email_unverified"), value: "unverified" },
	] satisfies ReadonlyArray<{
		label: string;
		value: "verified" | "unverified";
	}>;

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

		if (draftEmailVerified !== user.email_verified) {
			data.email_verified = draftEmailVerified;
		}
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

	const handleRefreshPolicyGroups = async () => {
		try {
			setPolicyGroupsLoading(true);
			setPolicyGroups(await loadAdminPolicyGroupLookup({ force: true }));
		} catch (e) {
			handleApiError(e);
		} finally {
			setPolicyGroupsLoading(false);
		}
	};

	return (
		<Dialog
			open={open}
			onOpenChange={onOpenChange}
			onOpenChangeComplete={handleOpenChangeComplete}
		>
			<DialogContent
				keepMounted
				className="flex max-h-[min(860px,calc(100vh-2rem))] flex-col gap-0 overflow-hidden p-0 sm:max-w-[min(1100px,calc(100vw-2rem))]"
			>
				<DialogHeader className="shrink-0 px-6 pt-5 pb-0 text-center max-lg:px-4 max-lg:pt-4">
					<DialogTitle className="text-lg">{t("user_details")}</DialogTitle>
				</DialogHeader>
				<div className="flex min-h-0 flex-1 flex-col overflow-y-auto lg:overflow-hidden">
					<div className="flex min-h-full flex-col lg:h-full lg:min-h-0 lg:flex-1 lg:flex-row">
						<UserDetailSidebar
							quota={quota}
							usagePercentage={pct}
							used={used}
							user={user}
						/>

						<div className="min-h-0 min-w-0 lg:flex-1 lg:overflow-y-auto">
							<div className="space-y-4 p-6 max-lg:p-4">
								<UserProfileSection
									draftEmailVerified={draftEmailVerified}
									draftRole={draftRole}
									draftStatus={draftStatus}
									emailVerificationOptions={emailVerificationOptions}
									isInitialAdmin={isInitialAdmin}
									onDraftEmailVerifiedChange={setDraftEmailVerified}
									onDraftRoleChange={setDraftRole}
									onDraftStatusChange={setDraftStatus}
									onQuotaValueChange={setQuotaValue}
									quotaValue={quotaValue}
									roleOptions={roleOptions}
									savingProfile={savingProfile}
									statusOptions={statusOptions}
									user={user}
								/>
								<UserPolicyGroupSection
									assignedPolicyGroupIsInvalid={assignedPolicyGroupIsInvalid}
									draftPolicyGroupId={draftPolicyGroupId}
									onDraftPolicyGroupIdChange={setDraftPolicyGroupId}
									onRefreshPolicyGroups={handleRefreshPolicyGroups}
									policyGroupOptions={policyGroupOptions}
									policyGroupsLoading={policyGroupsLoading}
									savingProfile={savingProfile}
								/>
								<UserSecurityActionsSection
									confirmPasswordValue={confirmPasswordValue}
									onConfirmPasswordValueChange={(value) => {
										setConfirmPasswordValue(value);
										setPasswordErrors((prev) => ({
											...prev,
											confirm: undefined,
										}));
									}}
									onPasswordReset={handlePasswordReset}
									onPasswordValueChange={(value) => {
										setPasswordValue(value);
										setPasswordErrors((prev) => ({
											...prev,
											password: undefined,
										}));
									}}
									onSessionRevoke={handleSessionRevoke}
									passwordErrors={passwordErrors}
									passwordValue={passwordValue}
									revokingSessions={revokingSessions}
									savingPassword={savingPassword}
								/>
							</div>
						</div>
					</div>
				</div>
				<DialogFooter className="mx-0 mb-0 w-full shrink-0 border-t bg-muted/10 px-6 py-4 max-lg:px-4 max-lg:py-3 sm:flex-row sm:items-center sm:justify-between">
					<p className="text-xs text-muted-foreground">
						{t("user_details_footer_hint")}
					</p>
					<div className="flex w-full flex-col-reverse gap-2 sm:w-auto sm:flex-row sm:items-center sm:justify-end">
						<Button variant="outline" onClick={() => onOpenChange(false)}>
							{t("core:close")}
						</Button>
						{hasProfileChanges ? (
							<Button
								onClick={() => void handleProfileSave()}
								disabled={savingProfile}
							>
								{savingProfile ? (
									<Icon name="Spinner" className="mr-1 size-4 animate-spin" />
								) : null}
								{t("save_changes")}
							</Button>
						) : null}
					</div>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
