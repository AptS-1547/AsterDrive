import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
	buildCreateManagedIngressProfilePayload,
	buildUpdateManagedIngressProfilePayload,
	emptyManagedIngressProfileForm,
	getManagedIngressProfileForm,
	type ManagedIngressProfileFormData,
} from "@/components/admin/managedIngressProfileDialogShared";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
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
import { Switch } from "@/components/ui/switch";
import { useConfirmDialog } from "@/hooks/useConfirmDialog";
import {
	ADMIN_CONTROL_HEIGHT_CLASS,
	ADMIN_ICON_BUTTON_CLASS,
} from "@/lib/constants";
import { formatBytes, formatDateTime } from "@/lib/format";
import type {
	RemoteCreateIngressProfileRequest,
	RemoteIngressProfileInfo,
	RemoteUpdateIngressProfileRequest,
} from "@/types/api";

interface RemoteNodeManagedIngressSectionProps {
	errorMessage: string | null;
	loading: boolean;
	onCreateProfile: (
		payload: RemoteCreateIngressProfileRequest,
	) => Promise<void>;
	onDeleteProfile: (profile: RemoteIngressProfileInfo) => Promise<void>;
	onUpdateProfile: (
		profileKey: string,
		payload: RemoteUpdateIngressProfileRequest,
	) => Promise<void>;
	profiles: RemoteIngressProfileInfo[];
}

function getProfileStatus(profile: RemoteIngressProfileInfo) {
	if (profile.last_error.trim()) {
		return {
			labelKey: "remote_node_ingress_profile_status_error",
			toneClass:
				"border-destructive/50 bg-destructive/10 text-destructive dark:border-destructive/40",
		};
	}

	if (profile.applied_revision < profile.desired_revision) {
		return {
			labelKey: "remote_node_ingress_profile_status_pending",
			toneClass:
				"border-amber-500/60 bg-amber-500/10 text-amber-700 dark:text-amber-300",
		};
	}

	return {
		labelKey: "remote_node_ingress_profile_status_ready",
		toneClass:
			"border-emerald-500/60 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
	};
}

function getDriverBadgeTone(
	driverType: RemoteIngressProfileInfo["driver_type"],
) {
	return driverType === "s3"
		? "border-blue-500/60 bg-blue-500/10 text-blue-700 dark:text-blue-300"
		: "border-slate-500/50 bg-slate-500/10 text-slate-700 dark:text-slate-300";
}

export function RemoteNodeManagedIngressSection({
	errorMessage,
	loading,
	onCreateProfile,
	onDeleteProfile,
	onUpdateProfile,
	profiles,
}: RemoteNodeManagedIngressSectionProps) {
	const { t } = useTranslation("admin");
	const [draftMode, setDraftMode] = useState<"create" | "edit" | null>(null);
	const [editingProfileKey, setEditingProfileKey] = useState<string | null>(
		null,
	);
	const [form, setForm] = useState<ManagedIngressProfileFormData>(
		emptyManagedIngressProfileForm,
	);
	const [submitting, setSubmitting] = useState(false);
	const editingProfile =
		draftMode === "edit"
			? (profiles.find(
					(profile) => profile.profile_key === editingProfileKey,
				) ?? null)
			: null;

	useEffect(() => {
		if (draftMode !== "edit" || editingProfileKey == null) {
			return;
		}

		if (
			!profiles.some((profile) => profile.profile_key === editingProfileKey)
		) {
			setDraftMode(null);
			setEditingProfileKey(null);
			setForm(emptyManagedIngressProfileForm);
		}
	}, [draftMode, editingProfileKey, profiles]);

	const startCreate = () => {
		setDraftMode("create");
		setEditingProfileKey(null);
		setForm({
			...emptyManagedIngressProfileForm,
			is_default: profiles.length === 0,
		});
	};

	const startEdit = (profile: RemoteIngressProfileInfo) => {
		setDraftMode("edit");
		setEditingProfileKey(profile.profile_key);
		setForm(getManagedIngressProfileForm(profile));
	};

	const resetDraft = () => {
		setDraftMode(null);
		setEditingProfileKey(null);
		setForm(emptyManagedIngressProfileForm);
	};

	const setField = <K extends keyof ManagedIngressProfileFormData>(
		key: K,
		value: ManagedIngressProfileFormData[K],
	) => setForm((current) => ({ ...current, [key]: value }));

	const nameError = form.name.trim()
		? null
		: t("remote_node_ingress_profile_name_required");
	const maxFileSizeValue = form.max_file_size.trim();
	const parsedMaxFileSize =
		maxFileSizeValue === "" ? 0 : Number(maxFileSizeValue);
	const maxFileSizeError =
		Number.isSafeInteger(parsedMaxFileSize) && parsedMaxFileSize >= 0
			? null
			: t("remote_node_ingress_profile_max_file_size_invalid");
	const localPathCandidate = form.base_path.trim().replaceAll("\\", "/");
	const localPathError =
		form.driver_type === "local"
			? !form.base_path.trim()
				? t("remote_node_ingress_profile_base_path_required")
				: localPathCandidate.startsWith("/") ||
						/^[A-Za-z]:/.test(localPathCandidate) ||
						localPathCandidate.split("/").some((segment) => segment === "..")
					? t("remote_node_ingress_profile_base_path_relative")
					: null
			: null;
	const endpointError =
		form.driver_type === "s3" && !form.endpoint.trim()
			? t("remote_node_ingress_profile_endpoint_required")
			: null;
	const bucketError =
		form.driver_type === "s3" && !form.bucket.trim()
			? t("remote_node_ingress_profile_bucket_required")
			: null;
	const requiresS3Credentials =
		form.driver_type === "s3" &&
		(draftMode === "create" || editingProfile?.driver_type !== "s3");
	const accessKeyError =
		requiresS3Credentials && !form.access_key.trim()
			? t("remote_node_ingress_profile_access_key_required")
			: null;
	const secretKeyError =
		requiresS3Credentials && !form.secret_key.trim()
			? t("remote_node_ingress_profile_secret_key_required")
			: null;
	const defaultToggleLocked =
		draftMode === "edit" && editingProfile?.is_default;
	const submitDisabled =
		submitting ||
		Boolean(errorMessage) ||
		Boolean(
			nameError ||
				maxFileSizeError ||
				localPathError ||
				endpointError ||
				bucketError ||
				accessKeyError ||
				secretKeyError,
		);

	const handleSubmit = async () => {
		if (draftMode == null || submitDisabled) {
			return;
		}

		setSubmitting(true);
		try {
			if (draftMode === "create") {
				await onCreateProfile(buildCreateManagedIngressProfilePayload(form));
			} else if (editingProfile != null) {
				await onUpdateProfile(
					editingProfile.profile_key,
					buildUpdateManagedIngressProfilePayload(form, editingProfile),
				);
			}
			resetDraft();
		} finally {
			setSubmitting(false);
		}
	};

	const {
		confirmId: deleteProfileKey,
		requestConfirm: requestDeleteConfirm,
		dialogProps: deleteDialogProps,
	} = useConfirmDialog<string>(async (profileKey) => {
		const profile = profiles.find((item) => item.profile_key === profileKey);
		if (!profile) {
			return;
		}
		await onDeleteProfile(profile);
		if (editingProfileKey === profileKey) {
			resetDraft();
		}
	});
	const deleteProfile =
		deleteProfileKey != null
			? (profiles.find((profile) => profile.profile_key === deleteProfileKey) ??
				null)
			: null;

	return (
		<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
			<div className="flex flex-wrap items-start justify-between gap-3">
				<div>
					<h3 className="text-base font-semibold text-foreground">
						{t("remote_node_ingress_profiles_title")}
					</h3>
					<p className="mt-1 text-sm text-muted-foreground">
						{t("remote_node_ingress_profiles_desc")}
					</p>
				</div>
				{draftMode == null ? (
					<Button
						type="button"
						size="sm"
						className={ADMIN_CONTROL_HEIGHT_CLASS}
						onClick={startCreate}
						disabled={loading || Boolean(errorMessage)}
					>
						<Icon name="Plus" className="mr-1 h-4 w-4" />
						{t("remote_node_ingress_profiles_create")}
					</Button>
				) : null}
			</div>

			{errorMessage ? (
				<div className="mt-4 rounded-2xl border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive">
					{errorMessage}
				</div>
			) : null}

			{draftMode != null ? (
				<div className="mt-4 rounded-2xl border border-border/70 bg-muted/10 p-4">
					<div className="flex flex-wrap items-start justify-between gap-3">
						<div>
							<h4 className="text-sm font-semibold text-foreground">
								{draftMode === "create"
									? t("remote_node_ingress_profile_form_create_title")
									: t("remote_node_ingress_profile_form_edit_title")}
							</h4>
							<p className="mt-1 text-xs leading-5 text-muted-foreground">
								{t("remote_node_ingress_profile_form_desc")}
							</p>
						</div>
						<Button
							type="button"
							variant="outline"
							size="sm"
							className={ADMIN_CONTROL_HEIGHT_CLASS}
							onClick={resetDraft}
							disabled={submitting}
						>
							{t("core:cancel")}
						</Button>
					</div>

					<div className="mt-4 grid gap-4 md:grid-cols-2">
						<div className="space-y-2">
							<Label htmlFor="managed-ingress-name">{t("core:name")}</Label>
							<Input
								id="managed-ingress-name"
								value={form.name}
								onChange={(event) => setField("name", event.target.value)}
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								aria-invalid={nameError ? true : undefined}
							/>
							{nameError ? (
								<p className="text-xs text-destructive">{nameError}</p>
							) : null}
						</div>

						<div className="space-y-2">
							<Label htmlFor="managed-ingress-driver">{t("driver_type")}</Label>
							<Select
								value={form.driver_type}
								onValueChange={(value) => {
									if (value === "local" || value === "s3") {
										setField("driver_type", value);
									}
								}}
							>
								<SelectTrigger
									id="managed-ingress-driver"
									className={ADMIN_CONTROL_HEIGHT_CLASS}
								>
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									<SelectItem value="local">
										{t("remote_node_ingress_profile_driver_local")}
									</SelectItem>
									<SelectItem value="s3">
										{t("remote_node_ingress_profile_driver_s3")}
									</SelectItem>
								</SelectContent>
							</Select>
						</div>

						<div className="space-y-2">
							<Label htmlFor="managed-ingress-base-path">
								{t("base_path")}
							</Label>
							<Input
								id="managed-ingress-base-path"
								value={form.base_path}
								onChange={(event) => setField("base_path", event.target.value)}
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								aria-invalid={localPathError ? true : undefined}
								placeholder={
									form.driver_type === "local" ? "tenant-a/incoming" : "prefix"
								}
							/>
							<p className="text-xs text-muted-foreground">
								{form.driver_type === "local"
									? t("remote_node_ingress_profile_local_path_hint")
									: t("remote_node_ingress_profile_s3_path_hint")}
							</p>
							{localPathError ? (
								<p className="text-xs text-destructive">{localPathError}</p>
							) : null}
						</div>

						<div className="space-y-2">
							<Label htmlFor="managed-ingress-max-file-size">
								{t("max_file_size")} (bytes)
							</Label>
							<Input
								id="managed-ingress-max-file-size"
								type="number"
								min="0"
								step="1"
								value={form.max_file_size}
								onChange={(event) =>
									setField("max_file_size", event.target.value)
								}
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								aria-invalid={maxFileSizeError ? true : undefined}
								placeholder="0"
							/>
							<p className="text-xs text-muted-foreground">
								{t("remote_node_ingress_profile_max_file_size_hint")}
							</p>
							{maxFileSizeError ? (
								<p className="text-xs text-destructive">{maxFileSizeError}</p>
							) : null}
						</div>

						{form.driver_type === "s3" ? (
							<>
								<div className="space-y-2">
									<Label htmlFor="managed-ingress-endpoint">
										{t("endpoint")}
									</Label>
									<Input
										id="managed-ingress-endpoint"
										value={form.endpoint}
										onChange={(event) =>
											setField("endpoint", event.target.value)
										}
										className={ADMIN_CONTROL_HEIGHT_CLASS}
										aria-invalid={endpointError ? true : undefined}
										placeholder="https://s3.example.com"
									/>
									{endpointError ? (
										<p className="text-xs text-destructive">{endpointError}</p>
									) : null}
								</div>

								<div className="space-y-2">
									<Label htmlFor="managed-ingress-bucket">{t("bucket")}</Label>
									<Input
										id="managed-ingress-bucket"
										value={form.bucket}
										onChange={(event) => setField("bucket", event.target.value)}
										className={ADMIN_CONTROL_HEIGHT_CLASS}
										aria-invalid={bucketError ? true : undefined}
									/>
									{bucketError ? (
										<p className="text-xs text-destructive">{bucketError}</p>
									) : null}
								</div>

								<div className="space-y-2">
									<Label htmlFor="managed-ingress-access-key">
										{t("access_key")}
									</Label>
									<Input
										id="managed-ingress-access-key"
										value={form.access_key}
										onChange={(event) =>
											setField("access_key", event.target.value)
										}
										className={ADMIN_CONTROL_HEIGHT_CLASS}
										aria-invalid={accessKeyError ? true : undefined}
									/>
									{accessKeyError ? (
										<p className="text-xs text-destructive">{accessKeyError}</p>
									) : null}
								</div>

								<div className="space-y-2">
									<Label htmlFor="managed-ingress-secret-key">
										{t("secret_key")}
									</Label>
									<Input
										id="managed-ingress-secret-key"
										type="password"
										value={form.secret_key}
										onChange={(event) =>
											setField("secret_key", event.target.value)
										}
										className={ADMIN_CONTROL_HEIGHT_CLASS}
										aria-invalid={secretKeyError ? true : undefined}
										placeholder={
											draftMode === "edit" &&
											editingProfile?.driver_type === "s3"
												? "••••••••"
												: undefined
										}
									/>
									<p className="text-xs text-muted-foreground">
										{draftMode === "edit" &&
										editingProfile?.driver_type === "s3"
											? t(
													"remote_node_ingress_profile_credentials_optional_hint",
												)
											: t("remote_node_ingress_profile_credentials_hint")}
									</p>
									{secretKeyError ? (
										<p className="text-xs text-destructive">{secretKeyError}</p>
									) : null}
								</div>
							</>
						) : (
							<div className="rounded-2xl border border-dashed border-border/70 bg-background/70 p-4 md:col-span-2">
								<p className="text-sm leading-6 text-muted-foreground">
									{t("remote_node_ingress_profile_local_scope_hint")}
								</p>
							</div>
						)}

						<div className="space-y-2 md:col-span-2">
							<div className="flex items-center gap-2">
								<Switch
									id="managed-ingress-default"
									checked={form.is_default}
									onCheckedChange={(value) => setField("is_default", value)}
									disabled={defaultToggleLocked}
								/>
								<Label htmlFor="managed-ingress-default">
									{t("remote_node_ingress_profile_default_toggle")}
								</Label>
							</div>
							<p className="text-xs text-muted-foreground">
								{defaultToggleLocked
									? t("remote_node_ingress_profile_default_locked_hint")
									: t("remote_node_ingress_profile_default_hint")}
							</p>
						</div>
					</div>

					<div className="mt-4 flex justify-end gap-2">
						<Button
							type="button"
							variant="outline"
							className={ADMIN_CONTROL_HEIGHT_CLASS}
							onClick={resetDraft}
							disabled={submitting}
						>
							{t("core:cancel")}
						</Button>
						<Button
							type="button"
							className={ADMIN_CONTROL_HEIGHT_CLASS}
							onClick={() => void handleSubmit()}
							disabled={submitDisabled}
						>
							<Icon
								name={submitting ? "Spinner" : "FloppyDisk"}
								className={`mr-1 h-4 w-4 ${submitting ? "animate-spin" : ""}`}
							/>
							{draftMode === "create" ? t("core:create") : t("save_changes")}
						</Button>
					</div>
				</div>
			) : null}

			<div className="mt-4 space-y-3">
				{errorMessage ? null : loading ? (
					<div className="rounded-2xl border border-border/70 bg-muted/10 p-4 text-sm text-muted-foreground">
						<span className="inline-flex items-center gap-2">
							<Icon name="Spinner" className="h-4 w-4 animate-spin" />
							{t("core:loading")}
						</span>
					</div>
				) : profiles.length === 0 ? (
					<div className="rounded-2xl border border-dashed border-border/70 bg-muted/10 p-4">
						<p className="text-sm font-medium text-foreground">
							{t("remote_node_ingress_profiles_empty")}
						</p>
						<p className="mt-1 text-sm text-muted-foreground">
							{t("remote_node_ingress_profiles_empty_desc")}
						</p>
					</div>
				) : (
					profiles.map((profile) => {
						const status = getProfileStatus(profile);
						return (
							<article
								key={profile.profile_key}
								className="rounded-2xl border border-border/70 bg-muted/10 p-4"
							>
								<div className="flex flex-wrap items-start justify-between gap-3">
									<div className="min-w-0">
										<div className="flex flex-wrap items-center gap-2">
											<h4 className="truncate text-sm font-semibold text-foreground">
												{profile.name}
											</h4>
											<Badge
												variant="outline"
												className={getDriverBadgeTone(profile.driver_type)}
											>
												{profile.driver_type === "s3"
													? t("remote_node_ingress_profile_driver_s3")
													: t("remote_node_ingress_profile_driver_local")}
											</Badge>
											{profile.is_default ? (
												<Badge
													variant="outline"
													className="border-blue-500/60 bg-blue-500/10 text-blue-700 dark:text-blue-300"
												>
													{t("remote_node_ingress_profile_default")}
												</Badge>
											) : null}
											<Badge variant="outline" className={status.toneClass}>
												{t(status.labelKey)}
											</Badge>
										</div>
										<p className="mt-1 break-all font-mono text-xs text-muted-foreground">
											{profile.profile_key}
										</p>
									</div>

									<div className="flex shrink-0 gap-1">
										<Button
											type="button"
											variant="ghost"
											size="icon"
											className={ADMIN_ICON_BUTTON_CLASS}
											onClick={() => startEdit(profile)}
											aria-label={t("core:edit")}
											title={t("core:edit")}
										>
											<Icon name="PencilSimple" className="h-3.5 w-3.5" />
										</Button>
										<Button
											type="button"
											variant="ghost"
											size="icon"
											className={`${ADMIN_ICON_BUTTON_CLASS} text-destructive`}
											onClick={() => requestDeleteConfirm(profile.profile_key)}
											aria-label={t("core:delete")}
											title={t("core:delete")}
										>
											<Icon name="Trash" className="h-3.5 w-3.5" />
										</Button>
									</div>
								</div>

								<dl className="mt-4 grid gap-3 text-sm md:grid-cols-2">
									<div>
										<dt className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
											{t("base_path")}
										</dt>
										<dd className="mt-1 break-all font-medium">
											{profile.base_path || "."}
										</dd>
									</div>
									<div>
										<dt className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
											{t("max_file_size")}
										</dt>
										<dd className="mt-1 font-medium">
											{profile.max_file_size > 0
												? formatBytes(profile.max_file_size)
												: t("core:unlimited")}
										</dd>
									</div>
									{profile.driver_type === "s3" ? (
										<>
											<div>
												<dt className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
													{t("endpoint")}
												</dt>
												<dd className="mt-1 break-all font-medium">
													{profile.endpoint}
												</dd>
											</div>
											<div>
												<dt className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
													{t("bucket")}
												</dt>
												<dd className="mt-1 break-all font-medium">
													{profile.bucket}
												</dd>
											</div>
										</>
									) : null}
									<div>
										<dt className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
											{t("remote_node_ingress_profile_revision")}
										</dt>
										<dd className="mt-1 font-medium">
											{profile.applied_revision} / {profile.desired_revision}
										</dd>
									</div>
									<div>
										<dt className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
											{t("core:updated_at")}
										</dt>
										<dd className="mt-1 font-medium">
											{formatDateTime(profile.updated_at)}
										</dd>
									</div>
								</dl>

								<div className="mt-4 rounded-2xl border border-border/70 bg-background/70 p-3">
									<div className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
										{t("remote_node_ingress_profile_last_error")}
									</div>
									<div className="mt-1 break-all text-sm">
										{profile.last_error ||
											t("remote_node_ingress_profile_last_error_empty")}
									</div>
								</div>
							</article>
						);
					})
				)}
			</div>

			<ConfirmDialog
				{...deleteDialogProps}
				title={
					deleteProfile
						? t("remote_node_ingress_profile_delete_title", {
								name: deleteProfile.name,
							})
						: t("remote_node_ingress_profile_delete_title", {
								name: "",
							})
				}
				description={t("remote_node_ingress_profile_delete_desc")}
				confirmLabel={t("core:delete")}
				variant="destructive"
			/>
		</section>
	);
}
