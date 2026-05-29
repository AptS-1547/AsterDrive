import { useTranslation } from "react-i18next";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import { cn } from "@/lib/utils";
import type { RemoteNodeInfo } from "@/types/api";
import type { RemoteNodeFormData } from "../remoteNodeDialogShared";
import {
	RemoteNodeDocsCard,
	RemoteNodeSectionIntro,
	RemoteNodeSummaryCard,
} from "./RemoteNodeDialogCards";
import type {
	RemoteNodeDialogStep,
	RemoteNodeFieldChangeHandler,
	RemoteNodeSummaryItem,
} from "./RemoteNodeDialogTypes";
import {
	type TransportModeOption,
	TransportModeSelector,
} from "./TransportModeSelector";

interface RemoteNodeCreateWizardProps {
	baseUrlValidationMessage: string | null;
	createNameError: string | null;
	createStep: number;
	createStepDirection: "idle" | "forward" | "backward";
	createSteps: RemoteNodeDialogStep[];
	editingNode: RemoteNodeInfo | null;
	enabledToneClass: string;
	form: RemoteNodeFormData;
	modeToneClass: string;
	onCreateStepChange: (step: number) => void;
	onFieldChange: RemoteNodeFieldChangeHandler;
	stepAnimationKey: string;
	summaryItems: RemoteNodeSummaryItem[];
	transportOptions: TransportModeOption[];
}

export function RemoteNodeCreateWizard({
	baseUrlValidationMessage,
	createNameError,
	createStep,
	createStepDirection,
	createSteps,
	editingNode,
	enabledToneClass,
	form,
	modeToneClass,
	onCreateStepChange,
	onFieldChange,
	stepAnimationKey,
	summaryItems,
	transportOptions,
}: RemoteNodeCreateWizardProps) {
	const { t } = useTranslation("admin");
	const createLastStep = createSteps.length - 1;
	const currentCreateStep = createSteps[Math.min(createStep, createLastStep)];
	const sideCards = (
		<div className="min-w-0 space-y-4 lg:sticky lg:top-0 lg:self-start">
			<RemoteNodeSummaryCard
				description={currentCreateStep.description}
				editingNode={editingNode}
				enabledToneClass={enabledToneClass}
				form={form}
				modeToneClass={modeToneClass}
				summaryItems={summaryItems}
			/>
			<RemoteNodeDocsCard />
		</div>
	);

	return (
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
									"flex items-center gap-3 rounded-2xl border p-3 text-left transition",
									index === createStep
										? "border-primary bg-primary/5"
										: index < createStep
											? "border-border/80 bg-background hover:border-primary/40"
											: "border-border/60 bg-background/70 text-muted-foreground",
								)}
							>
								<span className="flex size-6 shrink-0 items-center justify-center rounded-full border border-border/70 bg-background/80 text-[10px] font-semibold tracking-[0.16em] text-muted-foreground">
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
							key={stepAnimationKey}
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
											<RemoteNodeSectionIntro
												title={t("remote_node_overview_title")}
												description={t("remote_node_wizard_step_identity_desc")}
											/>
											<div className="grid gap-4">
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
														aria-invalid={createNameError ? true : undefined}
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
											</div>
										</section>
									</div>
									{sideCards}
								</div>
							) : createStep === 1 ? (
								<div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_280px]">
									<div className="min-w-0 space-y-4">
										<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
											<RemoteNodeSectionIntro
												title={t("remote_node_wizard_connection_block_title")}
												description={t(
													"remote_node_wizard_step_connection_desc",
												)}
											/>
											<div className="space-y-4">
												<div className="space-y-3">
													<Label>{t("remote_node_transport_mode")}</Label>
													<TransportModeSelector
														options={transportOptions}
														value={form.transport_mode}
														onChange={(value) =>
															onFieldChange("transport_mode", value)
														}
													/>
												</div>
												<div className="space-y-2">
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
												<div className="space-y-2 rounded-2xl border border-dashed border-border/70 bg-muted/10 p-4">
													<p className="text-[11px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
														{t("remote_node_wizard_auto_credentials_title")}
													</p>
													<p className="mt-2 text-sm leading-6 text-muted-foreground">
														{t("remote_node_wizard_auto_credentials_desc")}
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
									{sideCards}
								</div>
							) : (
								<div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_300px]">
									<div className="min-w-0 space-y-4">
										<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
											<RemoteNodeSectionIntro
												title={t("remote_node_wizard_step_review_title")}
												description={t("remote_node_wizard_step_review_desc")}
											/>
											<div className="grid gap-4 md:grid-cols-2">
												{summaryItems.map((item) => (
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
									</div>
									{sideCards}
								</div>
							)}
						</div>
					</div>
				</div>
			</div>
		</div>
	);
}
