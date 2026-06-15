import { useState } from "react";
import { toast } from "sonner";
import { AnimatedCollapsible } from "@/components/common/AnimatedCollapsible";
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
import { writeTextToClipboard } from "@/lib/clipboard";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import { cn } from "@/lib/utils";
import type {
	MicrosoftGraphCloud,
	OneDriveAccountMode,
	StoragePolicyCredentialInfo,
} from "@/types/api";
import type { SharedFieldProps, Translate } from "./StoragePolicyFieldTypes";

const MICROSOFT_GRAPH_PROVIDER = "microsoft_graph";
const ONE_DRIVE_CUSTOM_TENANT_MODE = "custom";
const ONE_DRIVE_AUTO_TENANT_MODE = "auto";
type OneDriveTenantMode =
	| typeof ONE_DRIVE_AUTO_TENANT_MODE
	| "consumers"
	| "organizations"
	| "common"
	| typeof ONE_DRIVE_CUSTOM_TENANT_MODE;

function getDefaultTenant(mode: OneDriveAccountMode) {
	if (mode === "personal") {
		return "consumers";
	}
	if (mode === "work_or_school") {
		return "common";
	}
	return "organizations";
}

function getTenantMode(form: SharedFieldProps["form"]): OneDriveTenantMode {
	const tenant = form.onedrive_tenant.trim();
	if (!tenant || tenant === getDefaultTenant(form.onedrive_account_mode)) {
		return ONE_DRIVE_AUTO_TENANT_MODE;
	}
	if (
		tenant === "consumers" ||
		tenant === "organizations" ||
		tenant === "common"
	) {
		return tenant;
	}
	return ONE_DRIVE_CUSTOM_TENANT_MODE;
}

function formatDateTime(value: string | null | undefined) {
	if (!value) {
		return null;
	}

	try {
		return new Intl.DateTimeFormat(undefined, {
			dateStyle: "medium",
			timeStyle: "short",
		}).format(new Date(value));
	} catch {
		return value;
	}
}

function OneDriveApplicationFields({
	clientIdError,
	form,
	onFieldChange,
	useSavedCredentialPlaceholder = false,
	showValidation = false,
	t,
}: SharedFieldProps & {
	clientIdError?: string | null;
	showValidation?: boolean;
	useSavedCredentialPlaceholder?: boolean;
}) {
	return (
		<div className="grid gap-4 md:grid-cols-2">
			<div className="space-y-2">
				<Label htmlFor="onedrive_client_id">{t("onedrive_client_id")}</Label>
				<Input
					id="onedrive_client_id"
					value={form.onedrive_client_id}
					onChange={(event) =>
						onFieldChange("onedrive_client_id", event.target.value)
					}
					aria-invalid={showValidation && clientIdError ? true : undefined}
					className={ADMIN_CONTROL_HEIGHT_CLASS}
					autoComplete="off"
					placeholder={
						useSavedCredentialPlaceholder
							? t("onedrive_client_id_keep_placeholder")
							: t("onedrive_client_id_placeholder")
					}
					required={showValidation}
				/>
				{showValidation && clientIdError ? (
					<p className="text-xs text-destructive">{clientIdError}</p>
				) : null}
			</div>
			<div className="space-y-2">
				<Label htmlFor="onedrive_client_secret">
					{t("onedrive_client_secret")}
				</Label>
				<Input
					id="onedrive_client_secret"
					type="password"
					value={form.onedrive_client_secret}
					onChange={(event) =>
						onFieldChange("onedrive_client_secret", event.target.value)
					}
					className={ADMIN_CONTROL_HEIGHT_CLASS}
					autoComplete="new-password"
					placeholder={
						useSavedCredentialPlaceholder
							? t("onedrive_client_secret_keep_placeholder")
							: t("onedrive_client_secret_optional")
					}
				/>
			</div>
		</div>
	);
}

export function OneDriveConnectionFields({
	clientIdError,
	form,
	mode = "edit",
	onFieldChange,
	showCreateValidation = false,
	t,
}: SharedFieldProps & {
	clientIdError?: string | null;
	mode?: "create" | "edit";
	showCreateValidation?: boolean;
}) {
	const cloudOptions = [
		{ label: t("onedrive_cloud_global"), value: "global" },
		{ label: t("onedrive_cloud_china"), value: "china" },
	] satisfies ReadonlyArray<{
		label: string;
		value: MicrosoftGraphCloud;
	}>;
	const accountModeOptions: Array<{
		label: string;
		value: OneDriveAccountMode;
	}> = [
		{
			label: t("onedrive_account_mode_work_or_school"),
			value: "work_or_school",
		},
		{
			label: t("onedrive_account_mode_sharepoint_site"),
			value: "sharepoint_site",
		},
		{ label: t("onedrive_account_mode_group_drive"), value: "group_drive" },
	];
	if (form.onedrive_cloud !== "china") {
		accountModeOptions.splice(1, 0, {
			label: t("onedrive_account_mode_personal"),
			value: "personal",
		});
	}
	const [advancedOpen, setAdvancedOpen] = useState(false);

	const targetFields =
		mode === "edit" ? (
			<>
				<div className="space-y-2">
					<Label htmlFor="onedrive_account_mode">
						{t("onedrive_account_mode")}
					</Label>
					<Select
						items={accountModeOptions}
						value={form.onedrive_account_mode}
						onValueChange={(value) => {
							const nextMode = (value ??
								"work_or_school") as OneDriveAccountMode;
							const tenantMode = getTenantMode(form);
							onFieldChange("onedrive_account_mode", nextMode);
							if (tenantMode === ONE_DRIVE_AUTO_TENANT_MODE) {
								onFieldChange("onedrive_tenant", getDefaultTenant(nextMode));
							}
						}}
					>
						<SelectTrigger id="onedrive_account_mode">
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							{accountModeOptions.map((option) => (
								<SelectItem key={option.value} value={option.value}>
									{option.label}
								</SelectItem>
							))}
						</SelectContent>
					</Select>
					<p className="text-xs leading-5 text-muted-foreground">
						{t("onedrive_account_mode_desc")}
					</p>
				</div>
				<div className="space-y-2">
					<Label htmlFor="onedrive_drive_id">{t("onedrive_drive_id")}</Label>
					<Input
						id="onedrive_drive_id"
						value={form.onedrive_drive_id}
						onChange={(event) =>
							onFieldChange("onedrive_drive_id", event.target.value)
						}
						className={ADMIN_CONTROL_HEIGHT_CLASS}
						placeholder={t("onedrive_drive_id_placeholder")}
					/>
					<p className="text-xs leading-5 text-muted-foreground">
						{t("onedrive_drive_id_desc")}
					</p>
				</div>

				<div className="space-y-2">
					<Label htmlFor="onedrive_root_item_id">
						{t("onedrive_root_item_id")}
					</Label>
					<Input
						id="onedrive_root_item_id"
						value={form.onedrive_root_item_id || "root"}
						onChange={(event) =>
							onFieldChange("onedrive_root_item_id", event.target.value)
						}
						className={ADMIN_CONTROL_HEIGHT_CLASS}
						placeholder="root"
					/>
					<p className="text-xs leading-5 text-muted-foreground">
						{t("onedrive_root_item_id_desc")}
					</p>
				</div>

				{form.onedrive_account_mode === "sharepoint_site" ? (
					<div className="space-y-2">
						<Label htmlFor="onedrive_site_id">{t("onedrive_site_id")}</Label>
						<Input
							id="onedrive_site_id"
							value={form.onedrive_site_id}
							onChange={(event) =>
								onFieldChange("onedrive_site_id", event.target.value)
							}
							className={ADMIN_CONTROL_HEIGHT_CLASS}
							placeholder="contoso.sharepoint.com,site-id,web-id"
						/>
						<p className="text-xs leading-5 text-muted-foreground">
							{t("onedrive_site_id_desc")}
						</p>
					</div>
				) : form.onedrive_account_mode === "group_drive" ? (
					<div className="space-y-2">
						<Label htmlFor="onedrive_group_id">{t("onedrive_group_id")}</Label>
						<Input
							id="onedrive_group_id"
							value={form.onedrive_group_id}
							onChange={(event) =>
								onFieldChange("onedrive_group_id", event.target.value)
							}
							className={ADMIN_CONTROL_HEIGHT_CLASS}
							placeholder="00000000-0000-0000-0000-000000000000"
						/>
						<p className="text-xs leading-5 text-muted-foreground">
							{t("onedrive_group_id_desc")}
						</p>
					</div>
				) : null}
			</>
		) : null;

	return (
		<div className="space-y-4">
			<div className="grid max-w-xl gap-4">
				<div className="space-y-2">
					<Label htmlFor="onedrive_cloud">{t("onedrive_cloud")}</Label>
					<Select
						items={cloudOptions}
						value={form.onedrive_cloud}
						onValueChange={(value) => {
							const nextCloud = (value ?? "global") as MicrosoftGraphCloud;
							onFieldChange("onedrive_cloud", nextCloud);
							onFieldChange(
								"onedrive_tenant",
								nextCloud === "china" ? "organizations" : "common",
							);
							if (
								nextCloud === "china" &&
								form.onedrive_account_mode === "personal"
							) {
								onFieldChange("onedrive_account_mode", "work_or_school");
								onFieldChange("onedrive_tenant", "organizations");
							}
						}}
					>
						<SelectTrigger id="onedrive_cloud">
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							{cloudOptions.map((option) => (
								<SelectItem key={option.value} value={option.value}>
									{option.label}
								</SelectItem>
							))}
						</SelectContent>
					</Select>
					<p className="text-xs leading-5 text-muted-foreground">
						{t("onedrive_cloud_desc")}
					</p>
				</div>
				{mode === "create" ? (
					<OneDriveApplicationFields
						clientIdError={clientIdError}
						form={form}
						showValidation={showCreateValidation}
						t={t}
						onFieldChange={onFieldChange}
					/>
				) : null}
			</div>
			{mode === "edit" ? (
				<div className="space-y-3">
					<Button
						type="button"
						variant="outline"
						className={cn(ADMIN_CONTROL_HEIGHT_CLASS, "w-fit")}
						onClick={() => setAdvancedOpen((open) => !open)}
					>
						<Icon name="Gear" className="mr-1 size-3.5" />
						{t("onedrive_advanced_target")}
						<Icon
							name={advancedOpen ? "CaretUp" : "CaretDown"}
							className="ml-1 size-3.5"
						/>
					</Button>
					<AnimatedCollapsible open={advancedOpen}>
						<div className="grid gap-4 rounded-lg border border-border/70 bg-muted/20 p-4 md:grid-cols-2">
							{targetFields}
						</div>
					</AnimatedCollapsible>
				</div>
			) : null}
		</div>
	);
}

export function OneDriveCredentialPanel({
	authorizationPending,
	credentials,
	form,
	loading,
	redirectUri,
	t,
	validationPending,
	onFieldChange,
	onStartAuthorization,
	onValidateCredential,
}: {
	authorizationPending: boolean;
	credentials: StoragePolicyCredentialInfo[];
	form: SharedFieldProps["form"];
	loading: boolean;
	redirectUri: string;
	t: Translate;
	validationPending: boolean;
	onFieldChange: SharedFieldProps["onFieldChange"];
	onStartAuthorization: () => void;
	onValidateCredential: () => void;
}) {
	const credential =
		credentials.find((item) => item.provider === MICROSOFT_GRAPH_PROVIDER) ??
		null;
	const status = credential?.status ?? "invalid";
	const statusClassName =
		status === "authorized"
			? "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
			: status === "reauth_required"
				? "border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300"
				: "border-destructive/30 bg-destructive/10 text-destructive";
	const authorizedAt = formatDateTime(credential?.authorized_at);
	const validatedAt = formatDateTime(credential?.last_validated_at);
	const copyRedirectUri = async () => {
		try {
			await writeTextToClipboard(redirectUri);
			toast.success(t("core:copied_to_clipboard"));
		} catch (error) {
			toast.error(error instanceof Error ? error.message : String(error));
		}
	};

	return (
		<div className="space-y-4 rounded-lg border border-sky-500/25 bg-sky-500/5 p-3">
			<div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
				<div className="min-w-0 space-y-1">
					<div className="flex flex-wrap items-center gap-2 text-sm font-medium">
						<Icon
							name="Key"
							className="size-4 shrink-0 text-sky-700 dark:text-sky-300"
						/>
						<span>{t("onedrive_credential_title")}</span>
						<Badge
							variant="outline"
							className={cn("shadow-sm", statusClassName)}
						>
							{loading
								? t("onedrive_credential_loading")
								: credential
									? t(`onedrive_credential_status_${credential.status}`)
									: t("onedrive_credential_status_missing")}
						</Badge>
					</div>
					<p className="text-xs leading-5 text-muted-foreground">
						{credential
							? t("onedrive_credential_desc_authorized")
							: t("onedrive_credential_desc_missing")}
					</p>
					{credential?.account_label || credential?.subject ? (
						<p className="text-xs text-muted-foreground">
							{credential.account_label ?? credential.subject}
						</p>
					) : null}
					{credential?.status_reason ? (
						<p className="text-xs text-amber-700 dark:text-amber-300">
							{credential.status_reason}
						</p>
					) : null}
					{authorizedAt || validatedAt ? (
						<p className="text-xs text-muted-foreground">
							{[
								authorizedAt
									? t("onedrive_credential_authorized_at", {
											time: authorizedAt,
										})
									: null,
								validatedAt
									? t("onedrive_credential_validated_at", {
											time: validatedAt,
										})
									: null,
							]
								.filter(Boolean)
								.join(" · ")}
						</p>
					) : null}
				</div>
				<div className="flex shrink-0 flex-wrap items-center gap-2">
					<Button
						type="button"
						variant="outline"
						className={ADMIN_CONTROL_HEIGHT_CLASS}
						disabled={authorizationPending}
						onClick={onStartAuthorization}
					>
						{authorizationPending ? (
							<Icon name="Spinner" className="mr-1 size-3.5 animate-spin" />
						) : (
							<Icon name="ArrowSquareOut" className="mr-1 size-3.5" />
						)}
						{credential
							? t("onedrive_reauthorize_action")
							: t("onedrive_authorize_action")}
					</Button>
					<Button
						type="button"
						variant="outline"
						className={ADMIN_CONTROL_HEIGHT_CLASS}
						disabled={!credential || validationPending}
						onClick={onValidateCredential}
					>
						{validationPending ? (
							<Icon name="Spinner" className="mr-1 size-3.5 animate-spin" />
						) : (
							<Icon name="Check" className="mr-1 size-3.5" />
						)}
						{t("onedrive_validate_action")}
					</Button>
				</div>
			</div>
			<div className="space-y-2">
				<Label htmlFor="onedrive_redirect_uri">
					{t("onedrive_redirect_uri")}
				</Label>
				<div className="flex gap-2">
					<Input
						id="onedrive_redirect_uri"
						readOnly
						value={redirectUri}
						className="font-mono text-xs"
					/>
					<Button
						type="button"
						variant="outline"
						size="icon"
						onClick={() => void copyRedirectUri()}
						aria-label={t("onedrive_copy_redirect_uri")}
						title={t("onedrive_copy_redirect_uri")}
					>
						<Icon name="Copy" className="size-4" />
					</Button>
				</div>
				<p className="text-xs leading-5 text-muted-foreground">
					{t("onedrive_redirect_uri_desc")}
				</p>
			</div>
			<OneDriveApplicationFields
				form={form}
				t={t}
				useSavedCredentialPlaceholder={credential != null}
				onFieldChange={onFieldChange}
			/>
		</div>
	);
}
