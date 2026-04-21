import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import { cn } from "@/lib/utils";
import type { RemoteNodeInfo } from "@/types/api";
import {
	getRemoteNodeBaseUrlValidationMessage,
	type RemoteNodeFormData,
} from "../remoteNodeDialogShared";
import { formatLastChecked, TestConnectionButton } from "./shared";

interface RemoteNodeDialogProps {
	createStep: number;
	createStepTouched: boolean;
	editingNode: RemoteNodeInfo | null;
	form: RemoteNodeFormData;
	mode: "create" | "edit";
	onCreateBack: () => void;
	onCreateNext: () => void;
	onCreateStepChange: (step: number) => void;
	onFieldChange: <K extends keyof RemoteNodeFormData>(
		key: K,
		value: RemoteNodeFormData[K],
	) => void;
	onOpenChange: (open: boolean) => void;
	onRunConnectionTest: () => Promise<boolean>;
	onSubmit: () => void;
	open: boolean;
	submitting: boolean;
}

export function RemoteNodeDialog({
	createStep,
	createStepTouched,
	editingNode,
	form,
	mode,
	onCreateBack,
	onCreateNext,
	onCreateStepChange,
	onFieldChange,
	onOpenChange,
	onRunConnectionTest,
	onSubmit,
	open,
	submitting,
}: RemoteNodeDialogProps) {
	const { t } = useTranslation("admin");
	const isCreateMode = mode === "create";
	const createSteps = [
		{
			title: t("remote_node_wizard_step_identity_title"),
			description: t("remote_node_wizard_step_identity_desc"),
		},
		{
			title: t("remote_node_wizard_step_connection_title"),
			description: t("remote_node_wizard_step_connection_desc"),
		},
		{
			title: t("remote_node_wizard_step_review_title"),
			description: t("remote_node_wizard_step_review_desc"),
		},
	];
	const createLastStep = createSteps.length - 1;
	const currentCreateStep = createSteps[Math.min(createStep, createLastStep)];
	const baseUrlValidationMessage = getRemoteNodeBaseUrlValidationMessage(
		form.base_url,
		t,
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
	const modeToneClass = form.base_url.trim()
		? "border-blue-500/60 bg-blue-500/10 text-blue-600 dark:text-blue-300"
		: "border-amber-500/60 bg-amber-500/10 text-amber-600 dark:text-amber-300";
	const enabledToneClass = form.is_enabled
		? "border-emerald-500/60 bg-emerald-500/10 text-emerald-600 dark:text-emerald-300"
		: "border-slate-500/40 bg-slate-500/10 text-slate-600 dark:text-slate-300";
	const remoteNodeModeLabel = form.base_url.trim()
		? t("remote_node_endpoint_configured")
		: t("remote_node_endpoint_pending");
	const hasConnectionFieldChanges =
		editingNode == null ? true : form.base_url !== editingNode.base_url;
	const canRunConnectionTest =
		editingNode !== null &&
		!hasConnectionFieldChanges &&
		Boolean(form.base_url.trim()) &&
		!baseUrlValidationMessage;
	const isSubmitDisabled =
		submitting ||
		!form.name.trim() ||
		!form.namespace.trim() ||
		Boolean(baseUrlValidationMessage);
	const createNameError =
		isCreateMode && createStep === 0 && createStepTouched && !form.name.trim()
			? t("remote_node_wizard_name_required")
			: null;
	const createNamespaceError =
		isCreateMode &&
		createStep === 0 &&
		createStepTouched &&
		!form.namespace.trim()
			? t("remote_node_wizard_namespace_required")
			: null;
	const createSummaryItems = [
		{
			label: t("namespace"),
			value: form.namespace || "—",
		},
		{
			label: t("base_url"),
			value: form.base_url || t("remote_node_base_url_empty"),
		},
		{
			label: t("remote_node_wizard_followup_label"),
			value: t("remote_node_wizard_followup_value"),
		},
		{
			label: t("remote_node_status"),
			value: form.is_enabled
				? t("remote_node_status_enabled")
				: t("remote_node_status_disabled"),
		},
	];

	const renderSectionIntro = (title: string, description: string) => (
		<div className="mb-5">
			<h3 className="text-base font-semibold text-foreground">{title}</h3>
			<p className="mt-1 text-sm text-muted-foreground">{description}</p>
		</div>
	);

	const renderCreateSummaryCard = (description: string) => (
		<section className="rounded-3xl border border-border/70 bg-muted/20 p-5">
			<div className="flex items-center gap-3">
				<div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-white shadow-sm ring-1 ring-black/5">
					<img
						src="/static/asterdrive/asterdrive-dark.svg"
						alt=""
						className="max-h-7 w-auto object-contain"
					/>
				</div>
				<div className="min-w-0">
					<p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
						{t("remote_node_summary_title")}
					</p>
					<h3 className="mt-1 truncate text-base font-semibold">
						{form.name || t("new_remote_node")}
					</h3>
				</div>
			</div>
			<p className="mt-4 text-sm leading-6 text-muted-foreground">
				{description}
			</p>
			<div className="mt-4 flex flex-wrap gap-2">
				<Badge variant="outline" className={modeToneClass}>
					{remoteNodeModeLabel}
				</Badge>
				<Badge variant="outline" className={enabledToneClass}>
					{form.is_enabled
						? t("remote_node_status_enabled")
						: t("remote_node_status_disabled")}
				</Badge>
			</div>
			<dl className="mt-4 space-y-3 text-sm">
				{createSummaryItems.map((item) => (
					<div key={item.label}>
						<dt className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
							{item.label}
						</dt>
						<dd className="mt-1 break-all font-medium">{item.value}</dd>
					</div>
				))}
			</dl>
		</section>
	);

	const renderCreateFlowDiagram = () => (
		<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
			{renderSectionIntro(
				t("remote_node_wizard_flow_title"),
				t("remote_node_wizard_flow_desc"),
			)}
			<div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_96px_minmax(0,1fr)_96px_minmax(0,1fr)] md:items-center">
				<div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
					<p className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
						{t("remote_node_wizard_flow_current_node")}
					</p>
					<p className="mt-2 font-semibold text-foreground">
						{form.name || t("new_remote_node")}
					</p>
					<p className="mt-2 text-xs leading-5 text-muted-foreground">
						{t("remote_node_wizard_followup_value")}
					</p>
				</div>
				<div className="flex justify-center">
					<Badge
						variant="outline"
						className="rounded-full border-foreground/15 px-3 py-1 text-[11px] font-medium"
					>
						{t("remote_node_enrollment_step_arrow_issue")}
					</Badge>
				</div>
				<div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
					<p className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
						2
					</p>
					<p className="mt-2 font-semibold text-foreground">
						{t("remote_node_enrollment_step_run_title")}
					</p>
					<p className="mt-2 text-xs leading-5 text-muted-foreground">
						{t("remote_node_enrollment_step_run_desc")}
					</p>
				</div>
				<div className="flex justify-center">
					<Badge
						variant="outline"
						className="rounded-full border-foreground/15 px-3 py-1 text-[11px] font-medium"
					>
						{t("remote_node_enrollment_step_arrow_run")}
					</Badge>
				</div>
				<div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
					<p className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
						3
					</p>
					<p className="mt-2 font-semibold text-foreground">
						{t("remote_node_enrollment_step_restart_title")}
					</p>
					<p className="mt-2 text-xs leading-5 text-muted-foreground">
						{t("remote_node_enrollment_step_restart_desc")}
					</p>
				</div>
			</div>
		</section>
	);

	useEffect(() => {
		if (!open || !isCreateMode) {
			previousCreateStepRef.current = 0;
			stepAnimationRef.current = {
				direction: "idle",
				step: 0,
			};
			return;
		}

		previousCreateStepRef.current = createStep;
	}, [createStep, isCreateMode, open]);

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="flex max-h-[min(90vh,calc(100vh-2rem))] flex-col gap-0 overflow-hidden p-0 sm:max-w-[calc(100%-2rem)] lg:max-w-4xl">
				<DialogHeader className="shrink-0 px-6 pt-5 pb-0 pr-14">
					<DialogTitle>
						{isCreateMode ? t("create_remote_node") : t("edit_remote_node")}
					</DialogTitle>
					<DialogDescription>{t("remote_nodes_intro")}</DialogDescription>
				</DialogHeader>
				<form
					onSubmit={(event) => event.preventDefault()}
					autoComplete="off"
					className="flex min-h-0 flex-1 flex-col overflow-hidden"
				>
					<div className="min-h-0 flex-1 overflow-y-auto px-6 pt-6 pb-5">
						{isCreateMode ? (
							<div className="space-y-6">
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
													{currentCreateStep.title}
												</h3>
												<p className="hidden text-sm text-muted-foreground sm:block">
													{currentCreateStep.description}
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
										<div className="mt-4 grid gap-2 md:grid-cols-3">
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
													<span className="text-sm font-medium leading-5">
														{step.title}
													</span>
												</button>
											))}
										</div>
									</div>

									<div className="rounded-2xl border border-border/70 bg-background/70 p-5">
										<div className="relative overflow-hidden">
											<div
												key={`${stepAnimationRef.current.step}-${stepAnimationRef.current.direction}`}
												data-testid="remote-node-step-panel"
												className={cn(
													createStepDirection === "idle"
														? undefined
														: "animate-in fade-in duration-[360ms] motion-reduce:animate-none",
													createStepDirection === "forward"
														? "slide-in-from-right-6"
														: createStepDirection === "backward"
															? "slide-in-from-left-6"
															: undefined,
												)}
											>
												{createStep === 0 ? (
													<div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_280px]">
														<div className="min-w-0 space-y-4">
															<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
																{renderSectionIntro(
																	t("remote_node_overview_title"),
																	t("remote_node_wizard_step_identity_desc"),
																)}
																<div className="grid gap-4 md:grid-cols-2">
																	<div className="space-y-2">
																		<Label htmlFor="remote-node-name">
																			{t("core:name")}
																		</Label>
																		<Input
																			id="remote-node-name"
																			value={form.name}
																			onChange={(event) =>
																				onFieldChange(
																					"name",
																					event.target.value,
																				)
																			}
																			className={ADMIN_CONTROL_HEIGHT_CLASS}
																			aria-invalid={
																				createNameError ? true : undefined
																			}
																			required
																		/>
																		<p className="text-xs text-muted-foreground">
																			{t("remote_node_name_hint")}
																		</p>
																		{createNameError ? (
																			<p className="text-xs text-destructive">
																				{createNameError}
																			</p>
																		) : null}
																	</div>
																	<div className="space-y-2">
																		<Label htmlFor="remote-node-namespace">
																			{t("namespace")}
																		</Label>
																		<Input
																			id="remote-node-namespace"
																			value={form.namespace}
																			onChange={(event) =>
																				onFieldChange(
																					"namespace",
																					event.target.value,
																				)
																			}
																			className={ADMIN_CONTROL_HEIGHT_CLASS}
																			aria-invalid={
																				createNamespaceError ? true : undefined
																			}
																			placeholder="tenant-a"
																			required
																		/>
																		<p className="text-xs text-muted-foreground">
																			{t("remote_node_namespace_hint")}
																		</p>
																		{createNamespaceError ? (
																			<p className="text-xs text-destructive">
																				{createNamespaceError}
																			</p>
																		) : null}
																	</div>
																</div>
															</section>
														</div>
														<div className="min-w-0 space-y-4 lg:sticky lg:top-0 lg:self-start">
															{renderCreateSummaryCard(
																currentCreateStep.description,
															)}
														</div>
													</div>
												) : createStep === 1 ? (
													<div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_280px]">
														<div className="min-w-0 space-y-4">
															<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
																{renderSectionIntro(
																	t(
																		"remote_node_wizard_connection_block_title",
																	),
																	t("remote_node_wizard_step_connection_desc"),
																)}
																<div className="space-y-4">
																	<div className="space-y-2">
																		<Label htmlFor="remote-node-base-url">
																			{t("base_url")}
																		</Label>
																		<Input
																			id="remote-node-base-url"
																			value={form.base_url}
																			onChange={(event) =>
																				onFieldChange(
																					"base_url",
																					event.target.value,
																				)
																			}
																			className={ADMIN_CONTROL_HEIGHT_CLASS}
																			aria-invalid={
																				baseUrlValidationMessage
																					? true
																					: undefined
																			}
																			placeholder="https://remote.example.com"
																		/>
																		<p className="text-xs text-muted-foreground">
																			{t("remote_node_base_url_hint")}
																		</p>
																		{baseUrlValidationMessage ? (
																			<p className="text-xs text-destructive">
																				{baseUrlValidationMessage}
																			</p>
																		) : null}
																	</div>
																	<div className="space-y-2 rounded-2xl border border-dashed border-border/70 bg-muted/10 p-4">
																		<p className="text-[11px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
																			{t(
																				"remote_node_wizard_auto_credentials_title",
																			)}
																		</p>
																		<p className="mt-2 text-sm leading-6 text-muted-foreground">
																			{t(
																				"remote_node_wizard_auto_credentials_desc",
																			)}
																		</p>
																	</div>
																	<div className="space-y-2">
																		<div className="flex items-center gap-2">
																			<Switch
																				id="remote-node-enabled"
																				checked={form.is_enabled}
																				onCheckedChange={(value) =>
																					onFieldChange("is_enabled", value)
																				}
																			/>
																			<Label htmlFor="remote-node-enabled">
																				{t("remote_node_enabled")}
																			</Label>
																		</div>
																		<p className="text-xs text-muted-foreground">
																			{t("remote_node_enabled_desc")}
																		</p>
																	</div>
																</div>
															</section>
														</div>
														<div className="min-w-0 space-y-4 lg:sticky lg:top-0 lg:self-start">
															{renderCreateSummaryCard(
																currentCreateStep.description,
															)}
														</div>
													</div>
												) : (
													<div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_300px]">
														<div className="min-w-0 space-y-4">
															<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
																{renderSectionIntro(
																	t("remote_node_wizard_step_review_title"),
																	t("remote_node_wizard_step_review_desc"),
																)}
																<div className="grid gap-4 md:grid-cols-2">
																	{createSummaryItems.map((item) => (
																		<div
																			key={item.label}
																			className="rounded-2xl border border-border/70 bg-muted/20 p-4"
																		>
																			<p className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
																				{item.label}
																			</p>
																			<p className="mt-2 break-all text-sm font-medium text-foreground">
																				{item.value}
																			</p>
																		</div>
																	))}
																</div>
															</section>
															{renderCreateFlowDiagram()}
														</div>
														<div className="min-w-0 space-y-4 lg:sticky lg:top-0 lg:self-start">
															{renderCreateSummaryCard(
																currentCreateStep.description,
															)}
															<section className="rounded-3xl border border-border/70 bg-background/85 p-5">
																<h3 className="text-sm font-semibold">
																	{t("remote_node_enrollment_command_title")}
																</h3>
																<p className="mt-1 text-xs leading-5 text-muted-foreground">
																	{t("remote_node_wizard_review_helper_desc")}
																</p>
															</section>
														</div>
													</div>
												)}
											</div>
										</div>
									</div>
								</div>
							</div>
						) : (
							<div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_280px]">
								<div className="min-w-0 space-y-4">
									<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
										{renderSectionIntro(
											t("remote_node_overview_title"),
											t("remote_node_overview_desc"),
										)}
										<div className="grid gap-4 md:grid-cols-2">
											<div className="space-y-2">
												<Label htmlFor="remote-node-name">
													{t("core:name")}
												</Label>
												<Input
													id="remote-node-name"
													value={form.name}
													onChange={(event) =>
														onFieldChange("name", event.target.value)
													}
													className={ADMIN_CONTROL_HEIGHT_CLASS}
													required
												/>
												<p className="text-xs text-muted-foreground">
													{t("remote_node_name_hint")}
												</p>
											</div>
											<div className="space-y-2">
												<Label htmlFor="remote-node-namespace">
													{t("namespace")}
												</Label>
												<Input
													id="remote-node-namespace"
													value={form.namespace}
													onChange={(event) =>
														onFieldChange("namespace", event.target.value)
													}
													className={ADMIN_CONTROL_HEIGHT_CLASS}
													placeholder="tenant-a"
													required
												/>
												<p className="text-xs text-muted-foreground">
													{t("remote_node_namespace_hint")}
												</p>
											</div>
											<div className="space-y-2 md:col-span-2">
												<Label htmlFor="remote-node-base-url">
													{t("base_url")}
												</Label>
												<Input
													id="remote-node-base-url"
													value={form.base_url}
													onChange={(event) =>
														onFieldChange("base_url", event.target.value)
													}
													className={ADMIN_CONTROL_HEIGHT_CLASS}
													aria-invalid={
														baseUrlValidationMessage ? true : undefined
													}
													placeholder="https://remote.example.com"
												/>
												<p className="text-xs text-muted-foreground">
													{t("remote_node_base_url_hint")}
												</p>
												{baseUrlValidationMessage ? (
													<p className="text-xs text-destructive">
														{baseUrlValidationMessage}
													</p>
												) : null}
											</div>
										</div>
									</section>

									<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
										{renderSectionIntro(
											t("remote_node_credentials_title"),
											t("remote_node_credentials_desc"),
										)}
										<div className="rounded-2xl border border-dashed border-border/70 bg-muted/10 p-4">
											<p className="text-sm leading-6 text-muted-foreground">
												{t("remote_node_wizard_auto_credentials_desc")}
											</p>
										</div>
									</section>

									<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
										{renderSectionIntro(
											t("remote_node_status_settings_title"),
											t("remote_node_status_settings_desc"),
										)}
										<div className="space-y-4">
											<div className="space-y-2">
												<div className="flex items-center gap-2">
													<Switch
														id="remote-node-enabled"
														checked={form.is_enabled}
														onCheckedChange={(value) =>
															onFieldChange("is_enabled", value)
														}
													/>
													<Label htmlFor="remote-node-enabled">
														{t("remote_node_enabled")}
													</Label>
												</div>
												<p className="text-xs text-muted-foreground">
													{t("remote_node_enabled_desc")}
												</p>
											</div>
										</div>
									</section>
								</div>

								<div className="min-w-0 space-y-4 lg:sticky lg:top-0 lg:self-start">
									{renderCreateSummaryCard(t("policy_editor_summary_desc"))}

									{editingNode ? (
										<section className="rounded-3xl border border-border/70 bg-background/85 p-5">
											<h3 className="text-sm font-semibold">
												{t("remote_node_diagnostics_title")}
											</h3>
											<p className="mt-1 text-xs text-muted-foreground">
												{t("remote_node_diagnostics_desc")}
											</p>
											<dl className="mt-4 space-y-3 text-sm">
												<div>
													<dt className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
														{t("remote_node_last_checked")}
													</dt>
													<dd className="mt-1 break-all font-medium">
														{formatLastChecked(t, editingNode.last_checked_at)}
													</dd>
												</div>
												<div>
													<dt className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
														{t("remote_node_last_error")}
													</dt>
													<dd className="mt-1 break-all font-medium">
														{editingNode.last_error ||
															t("remote_node_last_error_empty")}
													</dd>
												</div>
												<div>
													<dt className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
														{t("remote_node_capabilities")}
													</dt>
													<dd className="mt-1 space-y-1 text-xs text-muted-foreground">
														<div>
															{t("remote_node_protocol_version")}:{" "}
															{editingNode.capabilities.protocol_version}
														</div>
														<div>
															{t("remote_node_supports_list")}:{" "}
															{String(editingNode.capabilities.supports_list)}
														</div>
														<div>
															{t("remote_node_supports_range_read")}:{" "}
															{String(
																editingNode.capabilities.supports_range_read,
															)}
														</div>
														<div>
															{t("remote_node_supports_stream_upload")}:{" "}
															{String(
																editingNode.capabilities.supports_stream_upload,
															)}
														</div>
													</dd>
												</div>
											</dl>
										</section>
									) : null}
								</div>
							</div>
						)}
					</div>
					<DialogFooter className="mx-0 mb-0 w-full shrink-0 flex-row items-center gap-2 rounded-b-xl px-6 py-3">
						<div className="mr-auto flex shrink-0 gap-2">
							{isCreateMode && createStep > 0 ? (
								<Button
									type="button"
									variant="outline"
									className={ADMIN_CONTROL_HEIGHT_CLASS}
									onClick={onCreateBack}
									disabled={submitting}
								>
									{t("core:back")}
								</Button>
							) : null}
						</div>

						<div className="ml-auto flex shrink-0 flex-nowrap items-center justify-end gap-2">
							{isCreateMode ? (
								createStep === createLastStep ? (
									<Button
										type="button"
										className={ADMIN_CONTROL_HEIGHT_CLASS}
										disabled={isSubmitDisabled}
										onClick={onSubmit}
									>
										{t("remote_node_save_and_generate_enrollment_command")}
									</Button>
								) : (
									<Button
										type="button"
										className={ADMIN_CONTROL_HEIGHT_CLASS}
										onClick={onCreateNext}
										disabled={
											submitting ||
											(createStep === 1 && Boolean(baseUrlValidationMessage))
										}
									>
										{createStep === createLastStep - 1
											? t("policy_wizard_review")
											: t("policy_wizard_next")}
									</Button>
								)
							) : (
								<>
									<TestConnectionButton
										onTest={onRunConnectionTest}
										disabled={!canRunConnectionTest || submitting}
									/>
									<Button
										type="button"
										className={ADMIN_CONTROL_HEIGHT_CLASS}
										disabled={isSubmitDisabled}
										onClick={onSubmit}
									>
										{t("save_changes")}
									</Button>
								</>
							)}
						</div>
					</DialogFooter>
				</form>
			</DialogContent>
		</Dialog>
	);
}
