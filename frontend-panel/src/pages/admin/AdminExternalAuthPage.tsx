import type { TFunction } from "i18next";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
	ADMIN_INTERACTIVE_TABLE_ROW_CLASS,
	ADMIN_TABLE_BADGE_CELL_CLASS,
	ADMIN_TABLE_MONO_TEXT_CLASS,
	ADMIN_TABLE_MUTED_TEXT_CLASS,
	ADMIN_TABLE_STACKED_CELL_CLASS,
	ADMIN_TABLE_TEXT_CELL_CLASS,
	AdminTable,
	AdminTableBody,
	AdminTableShell,
	AdminTableCell as TableCell,
	AdminTableHead as TableHead,
	AdminTableHeader as TableHeader,
	AdminTableRow as TableRow,
} from "@/components/common/AdminTable";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { AdminPageHeader } from "@/components/layout/AdminPageHeader";
import { AdminPageShell } from "@/components/layout/AdminPageShell";
import { AdminSurface } from "@/components/layout/AdminSurface";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { handleApiError } from "@/hooks/useApiError";
import { useConfirmDialog } from "@/hooks/useConfirmDialog";
import { usePageTitle } from "@/hooks/usePageTitle";
import { writeTextToClipboard } from "@/lib/clipboard";
import {
	ADMIN_CONTROL_HEIGHT_CLASS,
	ADMIN_ICON_BUTTON_CLASS,
	ADMIN_TABLE_ACTIONS_WIDTH_CLASS,
} from "@/lib/constants";
import { formatDateAbsolute, formatDateAbsoluteWithOffset } from "@/lib/format";
import { absoluteAppUrl } from "@/lib/publicSiteUrl";
import { cn } from "@/lib/utils";
import { adminExternalAuthService } from "@/services/adminService";
import type {
	AdminExternalAuthProviderInfo,
	AdminExternalAuthProviderKindInfo,
	CreateExternalAuthProviderInput,
	ExternalAuthProviderKind,
	UpdateExternalAuthProviderInput,
} from "@/types/api";

const DEFAULT_SCOPES = "openid email profile";

interface ExternalAuthProviderFormData {
	allowedDomains: string;
	autoLinkVerifiedEmailEnabled: boolean;
	autoProvisionEnabled: boolean;
	clientId: string;
	clientSecret: string;
	displayName: string;
	displayNameClaim: string;
	emailClaim: string;
	enabled: boolean;
	groupsClaim: string;
	issuerUrl: string;
	key: string;
	providerKind: ExternalAuthProviderKind;
	requireEmailVerified: boolean;
	scopes: string;
	usernameClaim: string;
}

interface ExternalAuthCreateStep {
	title: string;
	description: string;
}

const emptyForm: ExternalAuthProviderFormData = {
	allowedDomains: "",
	autoLinkVerifiedEmailEnabled: false,
	autoProvisionEnabled: false,
	clientId: "",
	clientSecret: "",
	displayName: "",
	displayNameClaim: "",
	emailClaim: "",
	enabled: false,
	groupsClaim: "",
	issuerUrl: "",
	key: "",
	providerKind: "oidc",
	requireEmailVerified: true,
	scopes: DEFAULT_SCOPES,
	usernameClaim: "",
};

function formFromProvider(
	provider: AdminExternalAuthProviderInfo,
): ExternalAuthProviderFormData {
	return {
		allowedDomains: provider.allowed_domains.join(", "),
		autoLinkVerifiedEmailEnabled: provider.auto_link_verified_email_enabled,
		autoProvisionEnabled: provider.auto_provision_enabled,
		clientId: provider.client_id,
		clientSecret: provider.client_secret ?? "",
		displayName: provider.display_name,
		displayNameClaim: provider.display_name_claim ?? "",
		emailClaim: provider.email_claim ?? "",
		enabled: provider.enabled,
		groupsClaim: provider.groups_claim ?? "",
		issuerUrl: provider.issuer_url,
		key: provider.key,
		providerKind: provider.provider_kind,
		requireEmailVerified: provider.require_email_verified,
		scopes: provider.scopes || DEFAULT_SCOPES,
		usernameClaim: provider.username_claim ?? "",
	};
}

function kindFallbackLabel(kind: ExternalAuthProviderKind) {
	switch (kind) {
		case "oidc":
			return "OpenID Connect";
	}
}

function localizedProviderKindText(
	t: TFunction,
	key: string,
	fallback: string,
) {
	const translated = t(key);
	return translated === key ? fallback : translated;
}

function kindDisplayName(
	t: TFunction,
	kind: ExternalAuthProviderKind,
	providerKinds: AdminExternalAuthProviderKindInfo[],
) {
	const fallback =
		providerKinds.find((item) => item.kind === kind)?.display_name ??
		kindFallbackLabel(kind);
	return localizedProviderKindText(
		t,
		`external_auth_provider_kind_${kind}_name`,
		fallback,
	);
}

function kindDescription(
	t: TFunction,
	kind: AdminExternalAuthProviderKindInfo,
) {
	return localizedProviderKindText(
		t,
		`external_auth_provider_kind_${kind.kind}_description`,
		kind.description,
	);
}

function kindIconPath(kind: ExternalAuthProviderKind) {
	switch (kind) {
		case "oidc":
			return "/static/external-auth/openid-seeklogo.svg";
	}
}

function parseAllowedDomains(value: string) {
	return value
		.split(/[,\n]/)
		.map((domain) => domain.trim().replace(/^@+/, "").toLowerCase())
		.filter(
			(domain, index, domains) => domain && domains.indexOf(domain) === index,
		);
}

function nullableText(value: string) {
	const trimmed = value.trim();
	return trimmed ? trimmed : null;
}

function createPayload(
	form: ExternalAuthProviderFormData,
): CreateExternalAuthProviderInput {
	const allowedDomains = parseAllowedDomains(form.allowedDomains);
	return {
		allowed_domains: allowedDomains.length > 0 ? allowedDomains : null,
		auto_link_verified_email_enabled: form.autoLinkVerifiedEmailEnabled,
		auto_provision_enabled: form.autoProvisionEnabled,
		client_id: form.clientId.trim(),
		client_secret: nullableText(form.clientSecret),
		display_name: form.displayName.trim(),
		display_name_claim: nullableText(form.displayNameClaim),
		email_claim: nullableText(form.emailClaim),
		enabled: form.enabled,
		groups_claim: nullableText(form.groupsClaim),
		issuer_url: form.issuerUrl.trim(),
		key: form.key.trim(),
		provider_kind: form.providerKind,
		require_email_verified: form.requireEmailVerified,
		scopes: form.scopes.trim() || DEFAULT_SCOPES,
		username_claim: nullableText(form.usernameClaim),
	};
}

function updatePayload(
	form: ExternalAuthProviderFormData,
): UpdateExternalAuthProviderInput {
	const allowedDomains = parseAllowedDomains(form.allowedDomains);
	return {
		allowed_domains: allowedDomains.length > 0 ? allowedDomains : null,
		auto_link_verified_email_enabled: form.autoLinkVerifiedEmailEnabled,
		auto_provision_enabled: form.autoProvisionEnabled,
		client_id: form.clientId.trim(),
		client_secret: nullableText(form.clientSecret),
		display_name: form.displayName.trim(),
		display_name_claim: nullableText(form.displayNameClaim),
		email_claim: nullableText(form.emailClaim),
		enabled: form.enabled,
		groups_claim: nullableText(form.groupsClaim),
		issuer_url: form.issuerUrl.trim(),
		key: form.key.trim(),
		require_email_verified: form.requireEmailVerified,
		scopes: form.scopes.trim() || DEFAULT_SCOPES,
		username_claim: nullableText(form.usernameClaim),
	};
}

function providerStatusTone(provider: AdminExternalAuthProviderInfo) {
	return provider.enabled
		? "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950/60 dark:text-emerald-300"
		: "border-slate-200 bg-slate-50 text-slate-700 dark:border-slate-800 dark:bg-slate-950/50 dark:text-slate-300";
}

function securityModeLabel(
	t: TFunction,
	provider: AdminExternalAuthProviderInfo,
) {
	if (
		provider.auto_provision_enabled &&
		provider.auto_link_verified_email_enabled
	) {
		return t("external_auth_provider_mode_link_and_provision");
	}
	if (provider.auto_provision_enabled) {
		return t("external_auth_provider_mode_provision");
	}
	if (provider.auto_link_verified_email_enabled) {
		return t("external_auth_provider_mode_link");
	}
	return t("external_auth_provider_mode_manual");
}

function callbackPath(
	providerKind: ExternalAuthProviderKind,
	providerKey: string,
) {
	const key = providerKey.trim();
	return key
		? `/api/v1/auth/external-auth/${encodeURIComponent(providerKind)}/${encodeURIComponent(key)}/callback`
		: null;
}

function callbackUrl(
	providerKind: ExternalAuthProviderKind,
	providerKey: string,
) {
	const path = callbackPath(providerKind, providerKey);
	return path ? absoluteAppUrl(path) : "";
}

interface CreateProgressProps {
	createStep: number;
	createSteps: ExternalAuthCreateStep[];
	onCreateStepChange: (step: number) => void;
}

function CreateProgress({
	createStep,
	createSteps,
	onCreateStepChange,
}: CreateProgressProps) {
	const { t } = useTranslation("admin");
	const currentStep = createSteps[Math.min(createStep, createSteps.length - 1)];

	return (
		<div className="space-y-3">
			<div className="rounded-2xl border border-border/70 bg-muted/20 p-3 sm:p-4">
				<div className="flex items-start justify-between gap-3">
					<div className="space-y-1">
						<p className="text-[11px] font-medium uppercase tracking-[0.2em] text-muted-foreground">
							{t("policy_wizard_progress", {
								current: createStep + 1,
								total: createSteps.length,
							})}
						</p>
						<h3 className="text-sm font-semibold sm:text-base">
							{currentStep.title}
						</h3>
						<p className="hidden text-sm text-muted-foreground sm:block">
							{currentStep.description}
						</p>
					</div>
					<div className="hidden text-3xl leading-none font-semibold text-foreground/15 md:block">
						{String(createStep + 1).padStart(2, "0")}
					</div>
				</div>
				<div className="mt-4 h-1.5 overflow-hidden rounded-full bg-background/80">
					<div
						className="h-full rounded-full bg-primary transition-[width] duration-300"
						style={{
							width: `${((createStep + 1) / createSteps.length) * 100}%`,
						}}
					/>
				</div>
			</div>

			<div className="hidden gap-2 md:grid md:grid-cols-3">
				{createSteps.map((step, index) => (
					<button
						type="button"
						key={step.title}
						disabled={index > createStep}
						onClick={() => onCreateStepChange(index)}
						className={cn(
							"flex items-center gap-3 rounded-2xl border px-3 py-3 text-left transition",
							index === createStep
								? "border-primary bg-primary/5"
								: index < createStep
									? "border-border/80 bg-background hover:border-primary/40"
									: "border-border/60 bg-background/70 text-muted-foreground",
						)}
					>
						<span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-border/70 bg-background/80 text-[10px] font-semibold tracking-[0.16em] text-muted-foreground">
							{index + 1}
						</span>
						<span className="text-sm font-medium leading-5">{step.title}</span>
					</button>
				))}
			</div>
		</div>
	);
}

interface CallbackUrlFieldProps {
	className?: string;
	onCopy: (value: string) => void;
	value: string;
}

function CallbackUrlField({ className, onCopy, value }: CallbackUrlFieldProps) {
	const { t } = useTranslation("admin");
	const disabled = !value;

	return (
		<div
			className={cn(
				"flex min-w-0 items-center gap-2 rounded-md border border-border/70 bg-muted/30 p-1",
				className,
			)}
		>
			<code className="min-w-0 flex-1 truncate px-2 font-mono text-xs text-foreground">
				{value || "-"}
			</code>
			<Button
				type="button"
				variant="ghost"
				size="icon"
				className="h-7 w-7 shrink-0"
				disabled={disabled}
				aria-label={t("external_auth_provider_copy_callback_url")}
				title={t("external_auth_provider_copy_callback_url")}
				onClick={(event) => {
					event.stopPropagation();
					if (!disabled) {
						onCopy(value);
					}
				}}
			>
				<Icon name="Copy" className="h-3.5 w-3.5" />
			</Button>
		</div>
	);
}

interface ProviderDialogProps {
	createStep: number;
	createStepDirection: "idle" | "forward" | "backward";
	createStepTouched: boolean;
	createSteps: ExternalAuthCreateStep[];
	form: ExternalAuthProviderFormData;
	mode: "create" | "edit";
	onCreateBack: () => void;
	onCreateNext: () => void;
	onCreateStepChange: (step: number) => void;
	onFieldChange: <K extends keyof ExternalAuthProviderFormData>(
		key: K,
		value: ExternalAuthProviderFormData[K],
	) => void;
	onProviderKindChange: (kind: ExternalAuthProviderKind) => void;
	onCopyCallbackUrl: (value: string) => void;
	onOpenChange: (open: boolean) => void;
	onSubmit: () => void;
	open: boolean;
	provider: AdminExternalAuthProviderInfo | null;
	providerKinds: AdminExternalAuthProviderKindInfo[];
	submitting: boolean;
}

function ProviderDialog({
	createStep,
	createStepDirection,
	createStepTouched,
	createSteps,
	form,
	mode,
	onCreateBack,
	onCreateNext,
	onCreateStepChange,
	onCopyCallbackUrl,
	onFieldChange,
	onProviderKindChange,
	onOpenChange,
	onSubmit,
	open,
	provider,
	providerKinds,
	submitting,
}: ProviderDialogProps) {
	const { t } = useTranslation("admin");
	const isCreate = mode === "create";
	const createLastStep = createSteps.length - 1;
	const providerKind = provider?.provider_kind ?? form.providerKind;
	const selectedKind =
		providerKinds.find((item) => item.kind === providerKind) ??
		providerKinds[0] ??
		null;
	const providerKindLabel = kindDisplayName(t, providerKind, providerKinds);
	const stepAnimationKey = `${createStep}-${createStepDirection}`;
	const requiredMissing =
		!form.key.trim() ||
		!form.displayName.trim() ||
		!form.issuerUrl.trim() ||
		!form.clientId.trim();
	const currentCallbackUrl = callbackUrl(providerKind, form.key);
	const identityMissing =
		!form.key.trim() || !form.displayName.trim() || !form.issuerUrl.trim();
	const connectionMissing = !form.clientId.trim();
	const submitDisabled = submitting || requiredMissing;
	const stepPanelClass = cn(
		createStepDirection === "idle"
			? undefined
			: "animate-in fade-in duration-[360ms] motion-reduce:animate-none",
		createStepDirection === "forward"
			? "slide-in-from-right-6"
			: createStepDirection === "backward"
				? "slide-in-from-left-6"
				: undefined,
	);
	const accessPolicyPanel = (
		<section className="rounded-2xl border border-border/70 bg-muted/20 p-5">
			<h3 className="text-sm font-semibold">
				{t("external_auth_provider_access_title")}
			</h3>
			<div className="mt-4 space-y-4">
				<div className="space-y-2">
					<div className="flex items-center gap-2">
						<Switch
							id="external-auth-provider-enabled"
							checked={form.enabled}
							onCheckedChange={(value) => onFieldChange("enabled", value)}
						/>
						<Label htmlFor="external-auth-provider-enabled">
							{t("external_auth_provider_enabled")}
						</Label>
					</div>
					<p className="text-xs text-muted-foreground">
						{t("external_auth_provider_enabled_desc")}
					</p>
				</div>
				<div className="space-y-2">
					<div className="flex items-center gap-2">
						<Switch
							id="external-auth-provider-require-email-verified"
							checked={form.requireEmailVerified}
							onCheckedChange={(value) =>
								onFieldChange("requireEmailVerified", value)
							}
						/>
						<Label htmlFor="external-auth-provider-require-email-verified">
							{t("external_auth_provider_require_email_verified")}
						</Label>
					</div>
					<p className="text-xs text-muted-foreground">
						{t("external_auth_provider_require_email_verified_desc")}
					</p>
				</div>
				<div className="space-y-2">
					<div className="flex items-center gap-2">
						<Switch
							id="external-auth-provider-auto-link"
							checked={form.autoLinkVerifiedEmailEnabled}
							onCheckedChange={(value) =>
								onFieldChange("autoLinkVerifiedEmailEnabled", value)
							}
						/>
						<Label htmlFor="external-auth-provider-auto-link">
							{t("external_auth_provider_auto_link")}
						</Label>
					</div>
					<p className="text-xs text-muted-foreground">
						{t("external_auth_provider_auto_link_desc")}
					</p>
				</div>
				<div className="space-y-2">
					<div className="flex items-center gap-2">
						<Switch
							id="external-auth-provider-auto-provision"
							checked={form.autoProvisionEnabled}
							onCheckedChange={(value) =>
								onFieldChange("autoProvisionEnabled", value)
							}
						/>
						<Label htmlFor="external-auth-provider-auto-provision">
							{t("external_auth_provider_auto_provision")}
						</Label>
					</div>
					<p className="text-xs text-muted-foreground">
						{t("external_auth_provider_auto_provision_desc")}
					</p>
				</div>
			</div>
		</section>
	);
	const summaryPanel = (
		<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
			<h3 className="text-sm font-semibold">
				{t("external_auth_provider_summary_title")}
			</h3>
			<dl className="mt-4 space-y-3 text-sm">
				<div>
					<dt className="text-xs text-muted-foreground">
						{t("external_auth_provider_type")}
					</dt>
					<dd className="mt-1 text-xs font-medium">{providerKindLabel}</dd>
				</div>
				<div>
					<dt className="text-xs text-muted-foreground">
						{t("external_auth_provider_key")}
					</dt>
					<dd className="mt-1 font-mono text-xs">{form.key.trim() || "-"}</dd>
				</div>
				<div>
					<dt className="text-xs text-muted-foreground">
						{t("external_auth_provider_scopes")}
					</dt>
					<dd className="mt-1 text-xs">
						{form.scopes.trim() ||
							selectedKind?.default_scopes ||
							DEFAULT_SCOPES}
					</dd>
				</div>
				<div>
					<dt className="text-xs text-muted-foreground">
						{t("external_auth_provider_allowed_domains")}
					</dt>
					<dd className="mt-1 text-xs">
						{parseAllowedDomains(form.allowedDomains).join(", ") ||
							t("external_auth_provider_allowed_domains_all")}
					</dd>
				</div>
				<div>
					<dt className="text-xs text-muted-foreground">
						{t("external_auth_provider_callback_url")}
					</dt>
					<dd className="mt-1 break-all font-mono text-xs">
						{currentCallbackUrl || "-"}
					</dd>
				</div>
			</dl>
		</section>
	);

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="flex max-h-[min(90vh,calc(100vh-2rem))] flex-col gap-0 overflow-hidden p-0 sm:max-w-[calc(100%-2rem)] lg:max-w-4xl">
				<DialogHeader className="shrink-0 px-6 pt-5 pb-0 pr-14">
					<DialogTitle>
						{isCreate
							? t("external_auth_provider_create")
							: t("external_auth_provider_edit")}
					</DialogTitle>
					<DialogDescription>
						{t("external_auth_provider_dialog_desc")}
					</DialogDescription>
				</DialogHeader>
				<form
					onSubmit={(event) => {
						event.preventDefault();
						onSubmit();
					}}
					autoComplete="off"
					className="flex min-h-0 flex-1 flex-col overflow-hidden"
				>
					<div className="min-h-0 flex-1 overflow-y-auto px-6 pt-6 pb-5">
						{isCreate ? (
							<div className="space-y-6">
								<CreateProgress
									createStep={createStep}
									createSteps={createSteps}
									onCreateStepChange={onCreateStepChange}
								/>
								<div className="rounded-2xl border border-border/70 bg-background/70 p-5">
									<div className="relative overflow-hidden">
										<div
											key={stepAnimationKey}
											data-testid="external-auth-provider-step-panel"
											className={stepPanelClass}
										>
											{createStep === 0 ? (
												<div className="space-y-4">
													<div className="max-w-2xl">
														<h3 className="text-base font-semibold">
															{t(
																"external_auth_provider_wizard_choose_type_title",
															)}
														</h3>
														<p className="mt-1 text-sm text-muted-foreground">
															{t(
																"external_auth_provider_wizard_choose_type_desc",
															)}
														</p>
													</div>
													<div className="grid gap-4 md:grid-cols-2">
														{providerKinds.map((kind) => (
															<button
																type="button"
																key={kind.kind}
																aria-pressed={form.providerKind === kind.kind}
																onClick={() => onProviderKindChange(kind.kind)}
																className={cn(
																	"rounded-2xl border p-5 text-left transition",
																	form.providerKind === kind.kind
																		? "border-primary bg-primary/5 shadow-sm"
																		: "border-border bg-background hover:border-primary/40 hover:bg-muted/20",
																)}
															>
																<div className="flex items-start gap-4">
																	<div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-white shadow-xs ring-1 ring-black/5 dark:bg-slate-950 dark:ring-white/10">
																		{kindIconPath(kind.kind) ? (
																			<img
																				src={kindIconPath(kind.kind)}
																				alt=""
																				aria-hidden="true"
																				className="h-8 w-8 object-contain"
																			/>
																		) : (
																			<Icon
																				name="SignIn"
																				className="h-5 w-5 text-primary"
																			/>
																		)}
																	</div>
																	<div className="min-w-0 flex-1">
																		<div className="flex flex-wrap items-center gap-2">
																			<p className="text-base font-semibold">
																				{kindDisplayName(
																					t,
																					kind.kind,
																					providerKinds,
																				)}
																			</p>
																		</div>
																		<p className="mt-2 text-sm leading-6 text-muted-foreground">
																			{kindDescription(t, kind)}
																		</p>
																	</div>
																</div>
															</button>
														))}
													</div>
												</div>
											) : createStep === 1 ? (
												<div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_18rem]">
													<div className="min-w-0 space-y-4">
														<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
															<div className="space-y-1">
																<h3 className="text-sm font-semibold">
																	{t("external_auth_provider_identity_title")}
																</h3>
																<p className="text-sm text-muted-foreground">
																	{t("external_auth_provider_identity_desc")}
																</p>
															</div>
															<div className="mt-4 grid gap-4 md:grid-cols-2">
																<div className="space-y-2">
																	<Label htmlFor="external-auth-provider-key">
																		{t("external_auth_provider_key")}
																	</Label>
																	<Input
																		id="external-auth-provider-key"
																		value={form.key}
																		maxLength={64}
																		placeholder="authentik"
																		aria-invalid={
																			createStepTouched && !form.key.trim()
																				? true
																				: undefined
																		}
																		onChange={(event) =>
																			onFieldChange("key", event.target.value)
																		}
																	/>
																</div>
																<div className="space-y-2">
																	<Label htmlFor="external-auth-provider-display-name">
																		{t("external_auth_provider_display_name")}
																	</Label>
																	<Input
																		id="external-auth-provider-display-name"
																		value={form.displayName}
																		maxLength={128}
																		placeholder="Authentik"
																		aria-invalid={
																			createStepTouched &&
																			!form.displayName.trim()
																				? true
																				: undefined
																		}
																		onChange={(event) =>
																			onFieldChange(
																				"displayName",
																				event.target.value,
																			)
																		}
																	/>
																</div>
																<div className="space-y-2 md:col-span-2">
																	<Label htmlFor="external-auth-provider-issuer">
																		{t("external_auth_provider_issuer_url")}
																	</Label>
																	<Input
																		id="external-auth-provider-issuer"
																		value={form.issuerUrl}
																		placeholder="https://id.example.com/application/o/asterdrive/"
																		aria-invalid={
																			createStepTouched &&
																			!form.issuerUrl.trim()
																				? true
																				: undefined
																		}
																		onChange={(event) =>
																			onFieldChange(
																				"issuerUrl",
																				event.target.value,
																			)
																		}
																	/>
																</div>
																<div className="space-y-2 md:col-span-2">
																	<Label>
																		{t("external_auth_provider_callback_url")}
																	</Label>
																	<CallbackUrlField
																		value={currentCallbackUrl}
																		onCopy={onCopyCallbackUrl}
																	/>
																	<p className="text-xs text-muted-foreground">
																		{t(
																			"external_auth_provider_callback_url_hint",
																		)}
																	</p>
																</div>
																<div className="space-y-2">
																	<Label htmlFor="external-auth-provider-client-id">
																		{t("external_auth_provider_client_id")}
																	</Label>
																	<Input
																		id="external-auth-provider-client-id"
																		value={form.clientId}
																		aria-invalid={
																			createStepTouched && !form.clientId.trim()
																				? true
																				: undefined
																		}
																		onChange={(event) =>
																			onFieldChange(
																				"clientId",
																				event.target.value,
																			)
																		}
																	/>
																</div>
																<div className="space-y-2">
																	<Label htmlFor="external-auth-provider-client-secret">
																		{t("external_auth_provider_client_secret")}
																	</Label>
																	<Input
																		id="external-auth-provider-client-secret"
																		type="password"
																		value={form.clientSecret}
																		onChange={(event) =>
																			onFieldChange(
																				"clientSecret",
																				event.target.value,
																			)
																		}
																	/>
																	<p className="text-xs text-muted-foreground">
																		{t("external_auth_provider_secret_hint")}
																	</p>
																</div>
																{createStepTouched &&
																(identityMissing || connectionMissing) ? (
																	<p className="text-xs text-destructive md:col-span-2">
																		{t(
																			"external_auth_provider_wizard_required",
																		)}
																	</p>
																) : null}
															</div>
														</section>
													</div>
													<aside className="min-w-0 space-y-4 lg:sticky lg:top-0 lg:self-start">
														{summaryPanel}
													</aside>
												</div>
											) : (
												<div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_18rem]">
													<div className="min-w-0 space-y-4">
														<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
															<div className="space-y-1">
																<h3 className="text-sm font-semibold">
																	{t("external_auth_provider_rules_title")}
																</h3>
																<p className="text-sm text-muted-foreground">
																	{t("external_auth_provider_rules_desc")}
																</p>
															</div>
															<div className="mt-4 grid gap-4 md:grid-cols-2">
																<div className="space-y-2 md:col-span-2">
																	<Label htmlFor="external-auth-provider-scopes">
																		{t("external_auth_provider_scopes")}
																	</Label>
																	<Input
																		id="external-auth-provider-scopes"
																		value={form.scopes}
																		placeholder={
																			selectedKind?.default_scopes ??
																			DEFAULT_SCOPES
																		}
																		onChange={(event) =>
																			onFieldChange(
																				"scopes",
																				event.target.value,
																			)
																		}
																	/>
																</div>
																<div className="space-y-2 md:col-span-2">
																	<Label htmlFor="external-auth-provider-allowed-domains">
																		{t(
																			"external_auth_provider_allowed_domains",
																		)}
																	</Label>
																	<Input
																		id="external-auth-provider-allowed-domains"
																		value={form.allowedDomains}
																		placeholder="example.com, example.org"
																		onChange={(event) =>
																			onFieldChange(
																				"allowedDomains",
																				event.target.value,
																			)
																		}
																	/>
																</div>
																<div className="space-y-2">
																	<Label htmlFor="external-auth-provider-username-claim">
																		{t("external_auth_provider_username_claim")}
																	</Label>
																	<Input
																		id="external-auth-provider-username-claim"
																		value={form.usernameClaim}
																		placeholder="preferred_username"
																		onChange={(event) =>
																			onFieldChange(
																				"usernameClaim",
																				event.target.value,
																			)
																		}
																	/>
																</div>
																<div className="space-y-2">
																	<Label htmlFor="external-auth-provider-display-claim">
																		{t(
																			"external_auth_provider_display_name_claim",
																		)}
																	</Label>
																	<Input
																		id="external-auth-provider-display-claim"
																		value={form.displayNameClaim}
																		placeholder="name"
																		onChange={(event) =>
																			onFieldChange(
																				"displayNameClaim",
																				event.target.value,
																			)
																		}
																	/>
																</div>
																<div className="space-y-2">
																	<Label htmlFor="external-auth-provider-email-claim">
																		{t("external_auth_provider_email_claim")}
																	</Label>
																	<Input
																		id="external-auth-provider-email-claim"
																		value={form.emailClaim}
																		placeholder="email"
																		onChange={(event) =>
																			onFieldChange(
																				"emailClaim",
																				event.target.value,
																			)
																		}
																	/>
																</div>
																<div className="space-y-2">
																	<Label htmlFor="external-auth-provider-groups-claim">
																		{t("external_auth_provider_groups_claim")}
																	</Label>
																	<Input
																		id="external-auth-provider-groups-claim"
																		value={form.groupsClaim}
																		placeholder="groups"
																		onChange={(event) =>
																			onFieldChange(
																				"groupsClaim",
																				event.target.value,
																			)
																		}
																	/>
																</div>
															</div>
														</section>
														{accessPolicyPanel}
													</div>
													<aside className="min-w-0 space-y-4 lg:sticky lg:top-0 lg:self-start">
														{summaryPanel}
													</aside>
												</div>
											)}
										</div>
									</div>
								</div>
							</div>
						) : (
							<div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_18rem]">
								<div className="min-w-0 space-y-4">
									<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
										<div className="space-y-1">
											<h3 className="text-sm font-semibold">
												{t("external_auth_provider_identity_title")}
											</h3>
											<p className="text-sm text-muted-foreground">
												{t("external_auth_provider_identity_desc")}
											</p>
										</div>
										<div className="mt-4 grid gap-4 md:grid-cols-2">
											<div className="space-y-2">
												<p className="text-sm font-medium">
													{t("external_auth_provider_type")}
												</p>
												<div className="flex h-9 items-center">
													<Badge variant="outline">{providerKindLabel}</Badge>
												</div>
											</div>
											<div className="space-y-2">
												<Label htmlFor="external-auth-provider-key">
													{t("external_auth_provider_key")}
												</Label>
												<Input
													id="external-auth-provider-key"
													value={form.key}
													maxLength={64}
													placeholder="authentik"
													onChange={(event) =>
														onFieldChange("key", event.target.value)
													}
												/>
											</div>
											<div className="space-y-2">
												<Label htmlFor="external-auth-provider-display-name">
													{t("external_auth_provider_display_name")}
												</Label>
												<Input
													id="external-auth-provider-display-name"
													value={form.displayName}
													maxLength={128}
													placeholder="Authentik"
													onChange={(event) =>
														onFieldChange("displayName", event.target.value)
													}
												/>
											</div>
											<div className="space-y-2 md:col-span-2">
												<Label htmlFor="external-auth-provider-issuer">
													{t("external_auth_provider_issuer_url")}
												</Label>
												<Input
													id="external-auth-provider-issuer"
													value={form.issuerUrl}
													placeholder="https://id.example.com/application/o/asterdrive/"
													onChange={(event) =>
														onFieldChange("issuerUrl", event.target.value)
													}
												/>
											</div>
											<div className="space-y-2 md:col-span-2">
												<Label>
													{t("external_auth_provider_callback_url")}
												</Label>
												<CallbackUrlField
													value={currentCallbackUrl}
													onCopy={onCopyCallbackUrl}
												/>
												<p className="text-xs text-muted-foreground">
													{t("external_auth_provider_callback_url_hint")}
												</p>
											</div>
											<div className="space-y-2">
												<Label htmlFor="external-auth-provider-client-id">
													{t("external_auth_provider_client_id")}
												</Label>
												<Input
													id="external-auth-provider-client-id"
													value={form.clientId}
													onChange={(event) =>
														onFieldChange("clientId", event.target.value)
													}
												/>
											</div>
											<div className="space-y-2">
												<Label htmlFor="external-auth-provider-client-secret">
													{t("external_auth_provider_client_secret")}
												</Label>
												<Input
													id="external-auth-provider-client-secret"
													type="password"
													value={form.clientSecret}
													placeholder={
														provider?.client_secret_configured
															? t(
																	"external_auth_provider_secret_keep_placeholder",
																)
															: ""
													}
													onChange={(event) =>
														onFieldChange("clientSecret", event.target.value)
													}
												/>
												<p className="text-xs text-muted-foreground">
													{provider?.client_secret_configured
														? t("external_auth_provider_secret_keep_hint")
														: t("external_auth_provider_secret_hint")}
												</p>
											</div>
										</div>
									</section>
									<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
										<div className="space-y-1">
											<h3 className="text-sm font-semibold">
												{t("external_auth_provider_rules_title")}
											</h3>
											<p className="text-sm text-muted-foreground">
												{t("external_auth_provider_rules_desc")}
											</p>
										</div>
										<div className="mt-4 grid gap-4 md:grid-cols-2">
											<div className="space-y-2 md:col-span-2">
												<Label htmlFor="external-auth-provider-scopes">
													{t("external_auth_provider_scopes")}
												</Label>
												<Input
													id="external-auth-provider-scopes"
													value={form.scopes}
													placeholder={
														selectedKind?.default_scopes ?? DEFAULT_SCOPES
													}
													onChange={(event) =>
														onFieldChange("scopes", event.target.value)
													}
												/>
											</div>
											<div className="space-y-2 md:col-span-2">
												<Label htmlFor="external-auth-provider-allowed-domains">
													{t("external_auth_provider_allowed_domains")}
												</Label>
												<Input
													id="external-auth-provider-allowed-domains"
													value={form.allowedDomains}
													placeholder="example.com, example.org"
													onChange={(event) =>
														onFieldChange("allowedDomains", event.target.value)
													}
												/>
											</div>
											<div className="space-y-2">
												<Label htmlFor="external-auth-provider-username-claim">
													{t("external_auth_provider_username_claim")}
												</Label>
												<Input
													id="external-auth-provider-username-claim"
													value={form.usernameClaim}
													placeholder="preferred_username"
													onChange={(event) =>
														onFieldChange("usernameClaim", event.target.value)
													}
												/>
											</div>
											<div className="space-y-2">
												<Label htmlFor="external-auth-provider-display-claim">
													{t("external_auth_provider_display_name_claim")}
												</Label>
												<Input
													id="external-auth-provider-display-claim"
													value={form.displayNameClaim}
													placeholder="name"
													onChange={(event) =>
														onFieldChange(
															"displayNameClaim",
															event.target.value,
														)
													}
												/>
											</div>
											<div className="space-y-2">
												<Label htmlFor="external-auth-provider-email-claim">
													{t("external_auth_provider_email_claim")}
												</Label>
												<Input
													id="external-auth-provider-email-claim"
													value={form.emailClaim}
													placeholder="email"
													onChange={(event) =>
														onFieldChange("emailClaim", event.target.value)
													}
												/>
											</div>
											<div className="space-y-2">
												<Label htmlFor="external-auth-provider-groups-claim">
													{t("external_auth_provider_groups_claim")}
												</Label>
												<Input
													id="external-auth-provider-groups-claim"
													value={form.groupsClaim}
													placeholder="groups"
													onChange={(event) =>
														onFieldChange("groupsClaim", event.target.value)
													}
												/>
											</div>
										</div>
									</section>
								</div>
								<aside className="min-w-0 space-y-4 lg:sticky lg:top-0 lg:self-start">
									{accessPolicyPanel}
									{summaryPanel}
								</aside>
							</div>
						)}
					</div>
					<DialogFooter className="mx-0 mb-0 w-full shrink-0 flex-row items-center gap-2 rounded-b-xl px-6 py-3">
						<div className="mr-auto flex shrink-0 gap-2">
							{isCreate && createStep > 0 ? (
								<Button
									type="button"
									variant="outline"
									className={ADMIN_CONTROL_HEIGHT_CLASS}
									disabled={submitting}
									onClick={onCreateBack}
								>
									{t("core:back")}
								</Button>
							) : (
								<Button
									type="button"
									variant="outline"
									className={ADMIN_CONTROL_HEIGHT_CLASS}
									disabled={submitting}
									onClick={() => onOpenChange(false)}
								>
									{t("core:cancel")}
								</Button>
							)}
						</div>
						<div className="ml-auto flex shrink-0 flex-nowrap items-center justify-end gap-2">
							{isCreate && createStep < createLastStep ? (
								<Button
									type="button"
									className={ADMIN_CONTROL_HEIGHT_CLASS}
									disabled={submitting}
									onClick={onCreateNext}
								>
									{createStep === createLastStep - 1
										? t("policy_wizard_review")
										: t("policy_wizard_next")}
								</Button>
							) : (
								<Button
									type="submit"
									className={ADMIN_CONTROL_HEIGHT_CLASS}
									disabled={submitDisabled}
								>
									{submitting ? (
										<Icon
											name="Spinner"
											className="mr-2 h-4 w-4 animate-spin"
										/>
									) : (
										<Icon name="FloppyDisk" className="mr-2 h-4 w-4" />
									)}
									{isCreate
										? t("external_auth_provider_create")
										: t("save_changes")}
								</Button>
							)}
						</div>
					</DialogFooter>
				</form>
			</DialogContent>
		</Dialog>
	);
}

export default function AdminExternalAuthPage() {
	const { t } = useTranslation("admin");
	usePageTitle(t("external_auth"));
	const [providers, setProviders] = useState<AdminExternalAuthProviderInfo[]>(
		[],
	);
	const [providerKinds, setProviderKinds] = useState<
		AdminExternalAuthProviderKindInfo[]
	>([]);
	const [loading, setLoading] = useState(true);
	const [dialogOpen, setDialogOpen] = useState(false);
	const [editingProvider, setEditingProvider] =
		useState<AdminExternalAuthProviderInfo | null>(null);
	const [form, setForm] = useState<ExternalAuthProviderFormData>(emptyForm);
	const [createStep, setCreateStep] = useState(0);
	const [createStepTouched, setCreateStepTouched] = useState(false);
	const [submitting, setSubmitting] = useState(false);
	const [testingId, setTestingId] = useState<number | null>(null);
	const [deletingId, setDeletingId] = useState<number | null>(null);
	const [testResult, setTestResult] = useState<string | null>(null);
	const enabledCount = useMemo(
		() => providers.filter((provider) => provider.enabled).length,
		[providers],
	);
	const providerKindCount = providerKinds.length;
	const createSteps: ExternalAuthCreateStep[] = useMemo(
		() => [
			{
				title: t("external_auth_provider_wizard_step_type_title"),
				description: t("external_auth_provider_wizard_step_type_desc"),
			},
			{
				title: t("external_auth_provider_wizard_step_connection_title"),
				description: t("external_auth_provider_wizard_step_connection_desc"),
			},
			{
				title: t("external_auth_provider_wizard_step_rules_title"),
				description: t("external_auth_provider_wizard_step_rules_desc"),
			},
		],
		[t],
	);
	const previousCreateStepRef = useRef(createStep);
	const stepAnimationRef = useRef<{
		direction: "idle" | "forward" | "backward";
		step: number;
	}>({
		direction: "idle",
		step: createStep,
	});
	if (createStep !== previousCreateStepRef.current) {
		stepAnimationRef.current = {
			direction:
				createStep > previousCreateStepRef.current ? "forward" : "backward",
			step: createStep,
		};
	}
	const createStepDirection = stepAnimationRef.current.direction;

	const loadProviders = useCallback(async () => {
		try {
			setLoading(true);
			const [kinds, providerList] = await Promise.all([
				adminExternalAuthService.listKinds(),
				adminExternalAuthService.list(),
			]);
			setProviderKinds(kinds);
			setProviders(providerList);
		} catch (error) {
			handleApiError(error);
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		void loadProviders();
	}, [loadProviders]);

	useEffect(() => {
		if (!dialogOpen || editingProvider) {
			previousCreateStepRef.current = 0;
			stepAnimationRef.current = {
				direction: "idle",
				step: 0,
			};
			return;
		}

		previousCreateStepRef.current = createStep;
	}, [createStep, dialogOpen, editingProvider]);

	const setField = <K extends keyof ExternalAuthProviderFormData>(
		key: K,
		value: ExternalAuthProviderFormData[K],
	) => setForm((prev) => ({ ...prev, [key]: value }));

	const setProviderKind = (kind: ExternalAuthProviderKind) => {
		const descriptor = providerKinds.find((item) => item.kind === kind);
		setForm((prev) => ({
			...prev,
			providerKind: kind,
			scopes: descriptor?.default_scopes || prev.scopes || DEFAULT_SCOPES,
		}));
	};

	const copyCallbackUrl = async (value: string) => {
		try {
			await writeTextToClipboard(value);
			toast.success(t("core:copied_to_clipboard"));
		} catch {
			toast.error(t("errors:unexpected_error"));
		}
	};

	const openCreate = () => {
		setEditingProvider(null);
		const firstKind = providerKinds[0];
		setForm({
			...emptyForm,
			providerKind: firstKind?.kind ?? "oidc",
			scopes: firstKind?.default_scopes ?? DEFAULT_SCOPES,
		});
		setCreateStep(0);
		setCreateStepTouched(false);
		setTestResult(null);
		setDialogOpen(true);
		if (providerKinds.length === 0) {
			void adminExternalAuthService
				.listKinds()
				.then((kinds) => {
					setProviderKinds(kinds);
					const nextKind = kinds[0];
					if (nextKind) {
						setForm((prev) => ({
							...prev,
							providerKind: nextKind.kind,
							scopes: nextKind.default_scopes || DEFAULT_SCOPES,
						}));
					}
				})
				.catch(handleApiError);
		}
	};

	const openEdit = (provider: AdminExternalAuthProviderInfo) => {
		setEditingProvider(provider);
		setForm(formFromProvider(provider));
		setCreateStep(0);
		setCreateStepTouched(false);
		setTestResult(null);
		setDialogOpen(true);
	};

	const handleDialogOpenChange = (open: boolean) => {
		setDialogOpen(open);
		if (!open) {
			setEditingProvider(null);
			setForm(emptyForm);
			setCreateStep(0);
			setCreateStepTouched(false);
			setSubmitting(false);
		}
	};

	const canAdvanceCreateStep = () => {
		if (createStep === 0) {
			return providerKinds.length > 0;
		}
		if (createStep === 1) {
			return Boolean(
				form.key.trim() &&
					form.displayName.trim() &&
					form.issuerUrl.trim() &&
					form.clientId.trim(),
			);
		}
		return true;
	};

	const goCreateNext = () => {
		setCreateStepTouched(true);
		if (!canAdvanceCreateStep()) {
			return;
		}
		setCreateStep((step) => Math.min(step + 1, createSteps.length - 1));
		setCreateStepTouched(false);
	};

	const goCreateBack = () => {
		setCreateStep((step) => Math.max(step - 1, 0));
		setCreateStepTouched(false);
	};

	const goCreateStep = (step: number) => {
		setCreateStep(Math.max(0, Math.min(step, createSteps.length - 1)));
		setCreateStepTouched(false);
	};

	const submitProvider = async () => {
		if (submitting) return;

		setSubmitting(true);
		try {
			if (editingProvider) {
				const updated = await adminExternalAuthService.update(
					editingProvider.id,
					updatePayload(form),
				);
				setProviders((prev) =>
					prev.map((provider) =>
						provider.id === updated.id ? updated : provider,
					),
				);
				toast.success(t("external_auth_provider_updated"));
			} else {
				const created = await adminExternalAuthService.create(
					createPayload(form),
				);
				setProviders((prev) => [...prev, created]);
				toast.success(t("external_auth_provider_created"));
			}
			handleDialogOpenChange(false);
		} catch (error) {
			handleApiError(error);
		} finally {
			setSubmitting(false);
		}
	};

	const testProvider = async (provider: AdminExternalAuthProviderInfo) => {
		try {
			setTestingId(provider.id);
			const result = await adminExternalAuthService.test(provider.id);
			setTestResult(
				t("external_auth_provider_test_success_detail", {
					issuer: result.issuer,
					keys: result.jwks_key_count,
				}),
			);
			toast.success(t("external_auth_provider_test_success"));
			setProviders((prev) =>
				prev.map((item) =>
					item.id === provider.id
						? { ...item, updated_at: new Date().toISOString() }
						: item,
				),
			);
		} catch (error) {
			handleApiError(error);
		} finally {
			setTestingId(null);
		}
	};

	const deleteProvider = async (id: number) => {
		try {
			setDeletingId(id);
			await adminExternalAuthService.delete(id);
			setProviders((prev) => prev.filter((provider) => provider.id !== id));
			toast.success(t("external_auth_provider_deleted"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setDeletingId(null);
		}
	};

	const {
		confirmId: deleteId,
		requestConfirm,
		dialogProps,
	} = useConfirmDialog<number>(deleteProvider);
	const deleteProviderName =
		deleteId == null
			? ""
			: (providers.find((provider) => provider.id === deleteId)?.display_name ??
				"");

	return (
		<AdminLayout>
			<AdminPageShell>
				<AdminPageHeader
					title={t("external_auth")}
					description={t("external_auth_intro")}
					actions={
						<>
							<Button
								size="sm"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={openCreate}
							>
								<Icon name="Plus" className="mr-1 h-4 w-4" />
								{t("external_auth_provider_create")}
							</Button>
							<Button
								variant="outline"
								size="sm"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={() => void loadProviders()}
								disabled={loading}
							>
								<Icon
									name={loading ? "Spinner" : "ArrowsClockwise"}
									className={cn("mr-1 h-3.5 w-3.5", loading && "animate-spin")}
								/>
								{t("core:refresh")}
							</Button>
						</>
					}
				/>

				<div className="grid gap-4 md:grid-cols-3">
					<AdminSurface className="flex-none p-4">
						<p className="text-xs text-muted-foreground">
							{t("external_auth_providers_total")}
						</p>
						<p className="mt-1 text-2xl font-semibold">{providers.length}</p>
					</AdminSurface>
					<AdminSurface className="flex-none p-4">
						<p className="text-xs text-muted-foreground">
							{t("external_auth_providers_enabled")}
						</p>
						<p className="mt-1 text-2xl font-semibold">{enabledCount}</p>
					</AdminSurface>
					<AdminSurface className="flex-none p-4">
						<p className="text-xs text-muted-foreground">
							{t("external_auth_provider_kinds_supported")}
						</p>
						<p className="mt-1 text-2xl font-semibold">{providerKindCount}</p>
					</AdminSurface>
				</div>

				{testResult ? (
					<div className="rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800 dark:border-emerald-900 dark:bg-emerald-950/50 dark:text-emerald-200">
						{testResult}
					</div>
				) : null}

				{loading ? (
					<SkeletonTable columns={6} rows={6} />
				) : providers.length === 0 ? (
					<EmptyState
						icon={<Icon name="Globe" className="h-5 w-5" />}
						title={t("external_auth_providers_empty")}
						description={t("external_auth_providers_empty_desc")}
					/>
				) : (
					<AdminTableShell>
						<AdminTable>
							<TableHeader>
								<TableRow>
									<TableHead className="w-16">{t("id")}</TableHead>
									<TableHead>{t("external_auth_provider")}</TableHead>
									<TableHead>
										{t("external_auth_provider_issuer_url")}
									</TableHead>
									<TableHead>
										{t("external_auth_provider_callback_url")}
									</TableHead>
									<TableHead>{t("core:status")}</TableHead>
									<TableHead className={ADMIN_TABLE_ACTIONS_WIDTH_CLASS}>
										{t("core:actions")}
									</TableHead>
								</TableRow>
							</TableHeader>
							<AdminTableBody>
								{providers.map((provider) => {
									const deleting = deletingId === provider.id;
									const testing = testingId === provider.id;
									const providerCallbackUrl = callbackUrl(
										provider.provider_kind,
										provider.key,
									);
									return (
										<TableRow
											key={provider.id}
											className={ADMIN_INTERACTIVE_TABLE_ROW_CLASS}
											onClick={() => {
												if (!deleting) openEdit(provider);
											}}
											onKeyDown={(event) => {
												if (event.key === "Enter" || event.key === " ") {
													event.preventDefault();
													if (!deleting) openEdit(provider);
												}
											}}
											tabIndex={0}
										>
											<TableCell>
												<div className={ADMIN_TABLE_TEXT_CELL_CLASS}>
													<span className={ADMIN_TABLE_MONO_TEXT_CLASS}>
														{provider.id}
													</span>
												</div>
											</TableCell>
											<TableCell>
												<div className={ADMIN_TABLE_STACKED_CELL_CLASS}>
													<span className="truncate font-medium text-foreground">
														{provider.display_name}
													</span>
													<span className="flex min-w-0 flex-wrap items-center gap-2">
														<span className={ADMIN_TABLE_MONO_TEXT_CLASS}>
															{provider.key}
														</span>
														<Badge variant="outline">
															{kindDisplayName(
																t,
																provider.provider_kind,
																providerKinds,
															)}
														</Badge>
													</span>
												</div>
											</TableCell>
											<TableCell>
												<div className={ADMIN_TABLE_STACKED_CELL_CLASS}>
													<span className="truncate font-mono text-xs">
														{provider.issuer_url}
													</span>
													<span
														className={ADMIN_TABLE_MUTED_TEXT_CLASS}
														title={formatDateAbsoluteWithOffset(
															provider.updated_at,
														)}
													>
														{t("core:updated_at")}:{" "}
														{formatDateAbsolute(provider.updated_at)}
													</span>
												</div>
											</TableCell>
											<TableCell>
												<CallbackUrlField
													value={providerCallbackUrl}
													onCopy={(value) => void copyCallbackUrl(value)}
												/>
											</TableCell>
											<TableCell>
												<div className={ADMIN_TABLE_BADGE_CELL_CLASS}>
													<Badge
														variant="outline"
														className={providerStatusTone(provider)}
													>
														{provider.enabled
															? t("external_auth_provider_enabled_badge")
															: t("external_auth_provider_disabled_badge")}
													</Badge>
													<Badge variant="outline">
														{securityModeLabel(t, provider)}
													</Badge>
												</div>
											</TableCell>
											<TableCell
												onClick={(event) => event.stopPropagation()}
												onKeyDown={(event) => event.stopPropagation()}
											>
												<div className="flex justify-end gap-1">
													<Button
														variant="ghost"
														size="icon"
														className={ADMIN_ICON_BUTTON_CLASS}
														onClick={() => void testProvider(provider)}
														disabled={testing || deleting}
														aria-label={t("external_auth_provider_test")}
														title={t("external_auth_provider_test")}
													>
														<Icon
															name={testing ? "Spinner" : "WifiHigh"}
															className={cn(
																"h-3.5 w-3.5",
																testing && "animate-spin",
															)}
														/>
													</Button>
													<Button
														variant="ghost"
														size="icon"
														className={`${ADMIN_ICON_BUTTON_CLASS} text-destructive`}
														onClick={() => requestConfirm(provider.id)}
														disabled={deleting || testing}
														aria-label={t("external_auth_provider_delete")}
														title={t("external_auth_provider_delete")}
													>
														<Icon
															name={deleting ? "Spinner" : "Trash"}
															className={cn(
																"h-3.5 w-3.5",
																deleting && "animate-spin",
															)}
														/>
													</Button>
												</div>
											</TableCell>
										</TableRow>
									);
								})}
							</AdminTableBody>
						</AdminTable>
					</AdminTableShell>
				)}

				<ProviderDialog
					createStep={createStep}
					createStepDirection={createStepDirection}
					createStepTouched={createStepTouched}
					createSteps={createSteps}
					form={form}
					mode={editingProvider ? "edit" : "create"}
					onCreateBack={goCreateBack}
					onCreateNext={goCreateNext}
					onCreateStepChange={goCreateStep}
					open={dialogOpen}
					provider={editingProvider}
					providerKinds={providerKinds}
					submitting={submitting}
					onCopyCallbackUrl={(value) => void copyCallbackUrl(value)}
					onFieldChange={setField}
					onOpenChange={handleDialogOpenChange}
					onProviderKindChange={setProviderKind}
					onSubmit={() => void submitProvider()}
				/>

				<ConfirmDialog
					{...dialogProps}
					title={t("external_auth_provider_delete_title", {
						name: deleteProviderName,
					})}
					description={t("external_auth_provider_delete_desc")}
					confirmLabel={t("core:delete")}
					variant="destructive"
				/>
			</AdminPageShell>
		</AdminLayout>
	);
}
