import { type FormEvent, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useLocation, useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { SettingsSection } from "@/components/common/SettingsScaffold";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { handleApiError } from "@/hooks/useApiError";
import {
	clearContactVerificationRedirectSearch,
	getContactVerificationRedirectState,
} from "@/lib/contactVerificationRedirect";
import { emailSchema, passwordSchema } from "@/lib/validation";
import { authService } from "@/services/authService";
import { useAuthStore } from "@/stores/authStore";

type FormErrors = Partial<
	Record<
		"confirmPassword" | "currentPassword" | "email" | "newPassword",
		string
	>
>;

export function SecuritySettingsView() {
	const { t } = useTranslation(["auth", "core", "settings"]);
	const location = useLocation();
	const navigate = useNavigate();
	const user = useAuthStore((s) => s.user);
	const refreshUser = useAuthStore((s) => s.refreshUser);
	const syncSession = useAuthStore((s) => s.syncSession);
	const [emailBusy, setEmailBusy] = useState(false);
	const [newEmail, setNewEmail] = useState("");
	const [passwordBusy, setPasswordBusy] = useState(false);
	const [resendingEmailChange, setResendingEmailChange] = useState(false);
	const [currentPassword, setCurrentPassword] = useState("");
	const [newPassword, setNewPassword] = useState("");
	const [confirmPassword, setConfirmPassword] = useState("");
	const [errors, setErrors] = useState<FormErrors>({});

	useEffect(() => {
		const verification = getContactVerificationRedirectState(location.search);
		if (!verification) {
			return;
		}

		switch (verification.status) {
			case "email-changed":
				if (!verification.email) {
					return;
				}
				toast.success(
					t("settings:settings_email_change_confirmed", {
						email: verification.email,
					}),
					{
						id: `contact-verification-email-changed-settings:${verification.email}`,
					},
				);
				break;
			case "expired":
				toast.error(t("auth:verify_contact_expired_title"), {
					description: t("auth:verify_contact_expired_desc"),
					id: "contact-verification-expired-settings",
				});
				break;
			case "invalid":
				toast.error(t("auth:verify_contact_invalid_title"), {
					description: t("auth:verify_contact_invalid_desc"),
					id: "contact-verification-invalid-settings",
				});
				break;
			case "missing":
				toast.error(t("auth:verify_contact_missing_token_title"), {
					description: t("auth:verify_contact_missing_token_desc"),
					id: "contact-verification-missing-settings",
				});
				break;
			case "register-activated":
				toast.success(t("auth:activation_confirmed"), {
					id: "contact-verification-register-activated-settings",
				});
				break;
		}

		navigate(
			{
				hash: location.hash,
				pathname: location.pathname,
				search: clearContactVerificationRedirectSearch(location.search),
			},
			{ replace: true },
		);
	}, [location.hash, location.pathname, location.search, navigate, t]);

	const canSubmitPassword =
		!passwordBusy &&
		currentPassword.length > 0 &&
		newPassword.length > 0 &&
		confirmPassword.length > 0;
	const canSubmitEmailChange =
		!emailBusy && !!user?.email_verified && newEmail.trim().length > 0;

	const validateEmailChange = () => {
		const email = newEmail.trim();
		const emailResult = emailSchema.safeParse(email);
		if (!emailResult.success) {
			setErrors((prev) => ({
				...prev,
				email: emailResult.error.issues[0]?.message ?? "",
			}));
			return false;
		}

		if (email === user?.email) {
			setErrors((prev) => ({
				...prev,
				email: t("settings:settings_email_change_same"),
			}));
			return false;
		}

		setErrors((prev) => ({ ...prev, email: undefined }));
		return true;
	};

	const validate = () => {
		const nextErrors: FormErrors = {};
		const currentResult = passwordSchema.safeParse(currentPassword);
		if (!currentResult.success) {
			nextErrors.currentPassword = currentResult.error.issues[0]?.message ?? "";
		}

		const newResult = passwordSchema.safeParse(newPassword);
		if (!newResult.success) {
			nextErrors.newPassword = newResult.error.issues[0]?.message ?? "";
		}

		const confirmResult = passwordSchema.safeParse(confirmPassword);
		if (!confirmResult.success) {
			nextErrors.confirmPassword = confirmResult.error.issues[0]?.message ?? "";
		} else if (confirmPassword !== newPassword) {
			nextErrors.confirmPassword = t(
				"settings:settings_password_confirm_mismatch",
			);
		}

		setErrors(nextErrors);
		return Object.keys(nextErrors).length === 0;
	};

	const handleEmailChangeSubmit = async (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (!user || !validateEmailChange()) return;

		try {
			setEmailBusy(true);
			await authService.requestEmailChange(newEmail.trim());
			setNewEmail("");
			setErrors((prev) => ({ ...prev, email: undefined }));
			await refreshUser();
			toast.success(t("settings:settings_email_change_requested"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setEmailBusy(false);
		}
	};

	const handleResendEmailChange = async () => {
		if (!user?.pending_email) return;

		try {
			setResendingEmailChange(true);
			await authService.resendEmailChange();
			toast.success(t("settings:settings_email_change_resent"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setResendingEmailChange(false);
		}
	};

	const handlePasswordSubmit = async (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (!validate()) return;

		try {
			setPasswordBusy(true);
			const session = await authService.changePassword({
				current_password: currentPassword,
				new_password: newPassword,
			});
			syncSession(session.expiresIn);
			setCurrentPassword("");
			setNewPassword("");
			setConfirmPassword("");
			setErrors({});
			toast.success(t("settings:settings_password_updated"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setPasswordBusy(false);
		}
	};

	return (
		<SettingsSection
			title={t("settings:settings_security")}
			description={t("settings:settings_security_desc")}
			contentClassName="pt-4"
		>
			<div className="grid gap-5 rounded-xl border bg-muted/20 p-4 lg:grid-cols-[minmax(0,1fr)_280px]">
				<div className="space-y-4">
					<form
						className="space-y-4 rounded-xl border bg-background p-4"
						onSubmit={(event) => void handleEmailChangeSubmit(event)}
					>
						<div className="space-y-1">
							<h3 className="text-sm font-semibold">
								{t("settings:settings_email_section")}
							</h3>
							<p className="text-sm text-muted-foreground">
								{t("settings:settings_email_section_desc")}
							</p>
						</div>

						<div className="grid gap-4 md:grid-cols-2">
							<div className="space-y-2">
								<Label htmlFor="current-email">
									{t("settings:settings_email_current")}
								</Label>
								<Input
									id="current-email"
									value={user?.email ?? ""}
									readOnly
									aria-label={t("settings:settings_email_current")}
								/>
							</div>

							<div className="space-y-2">
								<Label>{t("settings:settings_email_status")}</Label>
								<div className="flex min-h-10 items-center gap-2 rounded-lg border px-3 py-2">
									<Badge
										variant="outline"
										className={
											user?.email_verified
												? "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
												: "border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300"
										}
									>
										{user?.email_verified
											? t("settings:settings_email_verified")
											: t("settings:settings_email_unverified")}
									</Badge>
									<span className="text-xs text-muted-foreground">
										{user?.email_verified
											? t("settings:settings_email_status_verified_desc")
											: t("settings:settings_email_status_unverified_desc")}
									</span>
								</div>
							</div>
						</div>

						{user?.pending_email ? (
							<div className="rounded-xl border bg-muted/20 p-4">
								<div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
									<div className="space-y-1">
										<p className="text-sm font-medium">
											{t("settings:settings_email_pending")}
										</p>
										<p className="text-sm text-muted-foreground">
											{t("settings:settings_email_pending_desc", {
												email: user.pending_email,
											})}
										</p>
									</div>
									<Button
										type="button"
										variant="outline"
										disabled={resendingEmailChange}
										onClick={() => void handleResendEmailChange()}
									>
										{resendingEmailChange ? (
											<Icon
												name="Spinner"
												className="mr-2 h-4 w-4 animate-spin"
											/>
										) : (
											<Icon name="ArrowClockwise" className="mr-2 h-4 w-4" />
										)}
										{resendingEmailChange
											? t("settings:settings_email_change_resending")
											: t("settings:settings_email_change_resend")}
									</Button>
								</div>
							</div>
						) : null}

						<div className="space-y-2">
							<Label htmlFor="new-email">
								{t("settings:settings_email_new")}
							</Label>
							<Input
								id="new-email"
								type="email"
								value={newEmail}
								disabled={emailBusy}
								aria-invalid={errors.email ? true : undefined}
								aria-label={t("settings:settings_email_new")}
								autoComplete="email"
								placeholder="you@example.com"
								onChange={(event) => {
									setNewEmail(event.target.value);
									setErrors((prev) => ({ ...prev, email: undefined }));
								}}
							/>
							<p className="text-xs text-muted-foreground">
								{user?.email_verified
									? t("settings:settings_email_change_hint")
									: t("settings:settings_email_change_requires_verified")}
							</p>
							{errors.email ? (
								<p className="text-xs text-destructive">{errors.email}</p>
							) : null}
						</div>

						<div className="flex justify-end border-t pt-4">
							<Button
								type="submit"
								className="min-w-28"
								disabled={!canSubmitEmailChange}
							>
								{emailBusy ? (
									<Icon name="Spinner" className="mr-2 h-4 w-4 animate-spin" />
								) : null}
								{emailBusy
									? t("settings:settings_email_change_requesting")
									: t("settings:settings_email_change_request")}
							</Button>
						</div>
					</form>

					<form
						className="space-y-4 rounded-xl border bg-background p-4"
						onSubmit={(event) => void handlePasswordSubmit(event)}
					>
						<div className="space-y-1">
							<h3 className="text-sm font-semibold">
								{t("settings:settings_password_section")}
							</h3>
							<p className="text-sm text-muted-foreground">
								{t("settings:settings_password_section_desc")}
							</p>
						</div>

						<div className="space-y-2">
							<Label htmlFor="current-password">
								{t("settings:settings_password_current")}
							</Label>
							<Input
								id="current-password"
								type="password"
								value={currentPassword}
								disabled={passwordBusy}
								aria-invalid={errors.currentPassword ? true : undefined}
								aria-label={t("settings:settings_password_current")}
								autoComplete="current-password"
								onChange={(event) => {
									setCurrentPassword(event.target.value);
									setErrors((prev) => ({
										...prev,
										currentPassword: undefined,
									}));
								}}
							/>
							{errors.currentPassword ? (
								<p className="text-xs text-destructive">
									{errors.currentPassword}
								</p>
							) : null}
						</div>

						<div className="space-y-2">
							<Label htmlFor="new-password">
								{t("settings:settings_password_new")}
							</Label>
							<Input
								id="new-password"
								type="password"
								value={newPassword}
								disabled={passwordBusy}
								aria-invalid={errors.newPassword ? true : undefined}
								aria-label={t("settings:settings_password_new")}
								autoComplete="new-password"
								onChange={(event) => {
									setNewPassword(event.target.value);
									setErrors((prev) => ({ ...prev, newPassword: undefined }));
								}}
							/>
							<p className="text-xs text-muted-foreground">
								{t("settings:settings_password_hint")}
							</p>
							{errors.newPassword ? (
								<p className="text-xs text-destructive">{errors.newPassword}</p>
							) : null}
						</div>

						<div className="space-y-2">
							<Label htmlFor="confirm-password">
								{t("settings:settings_password_confirm")}
							</Label>
							<Input
								id="confirm-password"
								type="password"
								value={confirmPassword}
								disabled={passwordBusy}
								aria-invalid={errors.confirmPassword ? true : undefined}
								aria-label={t("settings:settings_password_confirm")}
								autoComplete="new-password"
								onChange={(event) => {
									setConfirmPassword(event.target.value);
									setErrors((prev) => ({
										...prev,
										confirmPassword: undefined,
									}));
								}}
							/>
							{errors.confirmPassword ? (
								<p className="text-xs text-destructive">
									{errors.confirmPassword}
								</p>
							) : null}
						</div>

						<div className="flex justify-end border-t pt-4">
							<Button
								type="submit"
								className="min-w-28"
								disabled={!canSubmitPassword}
							>
								{t("save")}
							</Button>
						</div>
					</form>
				</div>

				<div className="rounded-xl border bg-background p-4">
					<div className="flex items-start gap-3">
						<div className="rounded-lg bg-primary/10 p-2 text-primary">
							<Icon name="Shield" className="h-4 w-4" />
						</div>
						<div className="space-y-3">
							<div className="space-y-1">
								<p className="text-sm font-semibold">
									{t("settings:settings_security_account")}
								</p>
								<p className="text-sm text-muted-foreground">
									{t("settings:settings_security_account_desc", {
										username: user?.username ?? "",
									})}
								</p>
							</div>
							<div className="space-y-1 border-t pt-3">
								<p className="text-sm font-semibold">
									{t("settings:settings_email_summary")}
								</p>
								<p className="text-sm text-muted-foreground">
									{user?.pending_email
										? t("settings:settings_email_pending_desc", {
												email: user.pending_email,
											})
										: user?.email_verified
											? t("settings:settings_email_summary_verified", {
													email: user.email,
												})
											: t("settings:settings_email_summary_unverified", {
													email: user?.email ?? "",
												})}
								</p>
							</div>
							<div className="space-y-1 border-t pt-3">
								<p className="text-sm font-semibold">
									{t("settings:settings_security_session")}
								</p>
								<p className="text-sm text-muted-foreground">
									{t("settings:settings_security_session_desc")}
								</p>
							</div>
						</div>
					</div>
				</div>
			</div>
		</SettingsSection>
	);
}
