import type { FormEvent } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type {
	PolicyFormData,
	S3UploadStrategy,
} from "@/components/admin/storagePolicyDialogShared";
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
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import { cn } from "@/lib/utils";
import type { DriverType } from "@/types/api";

interface StoragePolicyDialogProps {
	open: boolean;
	mode: "create" | "edit";
	form: PolicyFormData;
	submitting: boolean;
	createStep: number;
	createStepTouched: boolean;
	endpointValidationMessage: string | null;
	onOpenChange: (open: boolean) => void;
	onSubmit: (e: FormEvent) => void;
	onRunConnectionTest: () => Promise<boolean>;
	onFieldChange: <K extends keyof PolicyFormData>(
		key: K,
		value: PolicyFormData[K],
	) => void;
	onDriverTypeChange: (driverType: DriverType) => void;
	onCreateBack: () => void;
	onCreateStepChange: (step: number) => void;
	onCreateNext: () => void;
	onSyncNormalizedS3Form: () => void;
}

function TestConnectionButton({
	onTest,
	disabled = false,
}: {
	onTest: () => Promise<boolean>;
	disabled?: boolean;
}) {
	const { t } = useTranslation("admin");
	const [testing, setTesting] = useState(false);
	const [result, setResult] = useState<boolean | null>(null);

	const handleTest = async () => {
		setTesting(true);
		setResult(null);
		const passed = await onTest();
		setResult(passed);
		setTesting(false);
	};

	return (
		<Button
			type="button"
			variant="outline"
			className={ADMIN_CONTROL_HEIGHT_CLASS}
			disabled={testing || disabled}
			onClick={handleTest}
		>
			{testing ? (
				<Icon name="Spinner" className="h-4 w-4 mr-1 animate-spin" />
			) : result === true ? (
				<Icon
					name="Check"
					className="h-4 w-4 mr-1 text-green-600 dark:text-green-400"
				/>
			) : (
				<Icon name="WifiHigh" className="h-4 w-4 mr-1" />
			)}
			{t("test_connection")}
		</Button>
	);
}

export function StoragePolicyDialog({
	open,
	mode,
	form,
	submitting,
	createStep,
	createStepTouched,
	endpointValidationMessage,
	onOpenChange,
	onSubmit,
	onRunConnectionTest,
	onFieldChange,
	onDriverTypeChange,
	onCreateBack,
	onCreateStepChange,
	onCreateNext,
	onSyncNormalizedS3Form,
}: StoragePolicyDialogProps) {
	const { t } = useTranslation("admin");
	const isCreateMode = mode === "create";
	const storageOptions: Array<{
		description: string;
		iconSrc: string;
		type: DriverType;
		title: string;
	}> = [
		{
			type: "local",
			title: t("driver_type_local"),
			description: t("policy_wizard_local_storage_desc"),
			iconSrc: "/static/storage/asterdrive-local.svg",
		},
		{
			type: "s3",
			title: t("driver_type_s3"),
			description: t("policy_wizard_s3_storage_desc"),
			iconSrc: "/static/storage/amazon-s3.svg",
		},
	];
	const createSteps = [
		{
			title: t("policy_wizard_step_storage_title"),
			description: t("policy_wizard_step_storage_desc"),
		},
		{
			title:
				form.driver_type === "s3"
					? t("policy_wizard_step_connection_title")
					: t("policy_wizard_step_local_title"),
			description:
				form.driver_type === "s3"
					? t("policy_wizard_step_connection_desc")
					: t("policy_wizard_step_local_desc"),
		},
		{
			title: t("policy_wizard_step_rules_title"),
			description: t("policy_wizard_step_rules_desc"),
		},
	];
	const createLastStep = createSteps.length - 1;
	const currentCreateStep = createSteps[Math.min(createStep, createLastStep)];
	const currentStorageOption =
		storageOptions.find((option) => option.type === form.driver_type) ??
		storageOptions[0];
	const createNameError =
		isCreateMode && createStep === 1 && createStepTouched && !form.name.trim()
			? t("policy_wizard_name_required")
			: null;
	const createBucketError =
		isCreateMode &&
		createStep === 1 &&
		createStepTouched &&
		form.driver_type === "s3" &&
		!form.bucket.trim()
			? t("policy_wizard_bucket_required")
			: null;
	const s3UploadStrategyLabel =
		form.s3_upload_strategy === "proxy_tempfile"
			? t("s3_upload_strategy_proxy_tempfile")
			: form.s3_upload_strategy === "relay_stream"
				? t("s3_upload_strategy_relay_stream")
				: t("s3_upload_strategy_presigned");
	const createSummaryItems = [
		{ label: t("driver_type"), value: currentStorageOption.title },
		{
			label: t("base_path"),
			value:
				form.base_path ||
				(form.driver_type === "local" ? "./data" : t("core:root")),
		},
		{
			label: t("max_file_size"),
			value:
				form.max_file_size === ""
					? t("core:unlimited")
					: `${form.max_file_size} bytes`,
		},
		{
			label: t("chunk_size"),
			value: `${form.chunk_size || "0"} MB`,
		},
		{
			label: t("set_as_default"),
			value: form.is_default
				? t("policy_wizard_enabled")
				: t("policy_wizard_disabled"),
		},
		...(form.driver_type === "s3"
			? [
					{
						label: t("endpoint"),
						value: form.endpoint || t("policy_wizard_default_endpoint"),
					},
					{ label: t("bucket"), value: form.bucket || "—" },
					{
						label: t("s3_upload_strategy"),
						value: s3UploadStrategyLabel,
					},
				]
			: []),
	];

	const renderNameField = (showCreateValidation = false) => (
		<div className="space-y-2">
			<Label htmlFor="name">{t("core:name")}</Label>
			<Input
				id="name"
				value={form.name}
				onChange={(e) => onFieldChange("name", e.target.value)}
				aria-invalid={
					showCreateValidation && createNameError ? true : undefined
				}
				className={ADMIN_CONTROL_HEIGHT_CLASS}
				required
			/>
			{showCreateValidation && createNameError ? (
				<p className="text-xs text-destructive">{createNameError}</p>
			) : null}
		</div>
	);

	const renderBasePathField = () => (
		<div className="space-y-2">
			<Label htmlFor="base_path">{t("base_path")}</Label>
			<Input
				id="base_path"
				value={form.base_path}
				onChange={(e) => onFieldChange("base_path", e.target.value)}
				className={ADMIN_CONTROL_HEIGHT_CLASS}
				placeholder={form.driver_type === "local" ? "./data" : "prefix/path"}
			/>
		</div>
	);

	const renderS3ConnectionFields = (showCreateValidation = false) => (
		<>
			<div className="space-y-2">
				<Label htmlFor="endpoint">{t("endpoint")}</Label>
				<Input
					id="endpoint"
					value={form.endpoint}
					onChange={(e) => onFieldChange("endpoint", e.target.value)}
					onBlur={onSyncNormalizedS3Form}
					aria-invalid={endpointValidationMessage ? true : undefined}
					className={ADMIN_CONTROL_HEIGHT_CLASS}
					placeholder="https://s3.amazonaws.com"
				/>
				{endpointValidationMessage ? (
					<p className="text-xs text-destructive">
						{endpointValidationMessage}
					</p>
				) : null}
				<p className="text-xs text-muted-foreground">{t("s3_endpoint_hint")}</p>
			</div>
			<div className="space-y-2">
				<Label htmlFor="bucket">{t("bucket")}</Label>
				<Input
					id="bucket"
					value={form.bucket}
					onChange={(e) => onFieldChange("bucket", e.target.value)}
					aria-invalid={
						showCreateValidation && createBucketError ? true : undefined
					}
					className={ADMIN_CONTROL_HEIGHT_CLASS}
					required
				/>
				{showCreateValidation && createBucketError ? (
					<p className="text-xs text-destructive">{createBucketError}</p>
				) : null}
			</div>
			<div className="grid grid-cols-2 gap-4">
				<div className="space-y-2">
					<Label htmlFor="access_key">{t("access_key")}</Label>
					<Input
						id="access_key"
						value={form.access_key}
						onChange={(e) => onFieldChange("access_key", e.target.value)}
						className={ADMIN_CONTROL_HEIGHT_CLASS}
					/>
				</div>
				<div className="space-y-2">
					<Label htmlFor="secret_key">{t("secret_key")}</Label>
					<Input
						id="secret_key"
						type="password"
						value={form.secret_key}
						onChange={(e) => onFieldChange("secret_key", e.target.value)}
						className={ADMIN_CONTROL_HEIGHT_CLASS}
					/>
				</div>
			</div>
		</>
	);

	const renderS3UploadStrategyField = () => (
		<div className="space-y-2 pt-1">
			<Label htmlFor="s3_upload_strategy">{t("s3_upload_strategy")}</Label>
			<Select
				value={form.s3_upload_strategy}
				onValueChange={(value) =>
					onFieldChange("s3_upload_strategy", value as S3UploadStrategy)
				}
			>
				<SelectTrigger
					id="s3_upload_strategy"
					className={ADMIN_CONTROL_HEIGHT_CLASS}
				>
					<SelectValue />
				</SelectTrigger>
				<SelectContent>
					<SelectItem value="proxy_tempfile">
						{t("s3_upload_strategy_proxy_tempfile")}
					</SelectItem>
					<SelectItem value="relay_stream">
						{t("s3_upload_strategy_relay_stream")}
					</SelectItem>
					<SelectItem value="presigned">
						{t("s3_upload_strategy_presigned")}
					</SelectItem>
				</SelectContent>
			</Select>
			<p className="text-xs text-muted-foreground">
				{t(
					`${
						form.s3_upload_strategy === "proxy_tempfile"
							? "s3_upload_strategy_proxy_tempfile_desc"
							: form.s3_upload_strategy === "relay_stream"
								? "s3_upload_strategy_relay_stream_desc"
								: "s3_upload_strategy_presigned_desc"
					}`,
				)}
			</p>
		</div>
	);

	const renderLimitsFields = () => (
		<>
			<div className="space-y-2">
				<Label htmlFor="max_file_size">{t("max_file_size")} (bytes)</Label>
				<Input
					id="max_file_size"
					type="number"
					value={form.max_file_size}
					onChange={(e) => onFieldChange("max_file_size", e.target.value)}
					className={ADMIN_CONTROL_HEIGHT_CLASS}
					placeholder={`0 = ${t("core:unlimited").toLowerCase()}`}
				/>
			</div>

			<div className="space-y-2">
				<Label htmlFor="chunk_size">{t("chunk_size")}</Label>
				<Input
					id="chunk_size"
					type="number"
					value={form.chunk_size}
					onChange={(e) => onFieldChange("chunk_size", e.target.value)}
					className={ADMIN_CONTROL_HEIGHT_CLASS}
					placeholder="5 = 5MB, 0 = single upload only"
				/>
				<p className="text-xs text-muted-foreground">{t("chunk_size_desc")}</p>
			</div>
		</>
	);

	const renderDefaultToggle = () => (
		<div className="flex items-center gap-2">
			<Switch
				id="is_default"
				checked={form.is_default}
				onCheckedChange={(value) => onFieldChange("is_default", value)}
			/>
			<Label htmlFor="is_default">{t("set_as_default")}</Label>
		</div>
	);

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent
				className={cn(
					"max-h-[90vh] overflow-y-auto",
					isCreateMode ? "sm:max-w-4xl" : "sm:max-w-lg",
				)}
			>
				<DialogHeader>
					<DialogTitle>
						{isCreateMode ? t("create_policy") : t("edit_policy")}
					</DialogTitle>
					<DialogDescription>
						{isCreateMode ? currentCreateStep.description : t("policies_intro")}
					</DialogDescription>
				</DialogHeader>
				<form onSubmit={onSubmit} className="space-y-6">
					{isCreateMode ? (
						<>
							<div className="space-y-4">
								<div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
									<div className="flex items-start justify-between gap-4">
										<div className="space-y-1">
											<p className="text-xs font-medium uppercase tracking-[0.22em] text-muted-foreground">
												{t("policy_wizard_progress", {
													current: createStep + 1,
													total: createSteps.length,
												})}
											</p>
											<h3 className="text-base font-semibold">
												{currentCreateStep.title}
											</h3>
											<p className="text-sm text-muted-foreground">
												{currentCreateStep.description}
											</p>
										</div>
										<div className="hidden text-4xl leading-none font-semibold text-foreground/15 sm:block">
											{String(createStep + 1).padStart(2, "0")}
										</div>
									</div>
									<div className="mt-4 h-1.5 overflow-hidden rounded-full bg-muted">
										<div
											className="h-full rounded-full bg-primary transition-all"
											style={{
												width: `${((createStep + 1) / createSteps.length) * 100}%`,
											}}
										/>
									</div>
								</div>

								<div className="grid gap-2 sm:grid-cols-3">
									{createSteps.map((step, index) => (
										<button
											type="button"
											key={step.title}
											disabled={index > createStep}
											onClick={() => onCreateStepChange(index)}
											className={cn(
												"rounded-2xl border px-4 py-3 text-left transition",
												index === createStep
													? "border-primary bg-primary/5 shadow-sm"
													: index < createStep
														? "border-border bg-background hover:border-primary/40"
														: "border-border/60 bg-muted/20 text-muted-foreground",
											)}
										>
											<span className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
												0{index + 1}
											</span>
											<p className="mt-2 text-sm font-medium">{step.title}</p>
										</button>
									))}
								</div>
							</div>

							<div className="rounded-2xl border border-border/70 bg-background/70 p-5">
								{createStep === 0 ? (
									<div className="space-y-4">
										<div className="max-w-2xl">
											<h3 className="text-base font-semibold">
												{t("policy_wizard_choose_driver_title")}
											</h3>
											<p className="mt-1 text-sm text-muted-foreground">
												{t("policy_wizard_choose_driver_desc")}
											</p>
										</div>
										<div className="grid gap-4 md:grid-cols-2">
											{storageOptions.map((option) => (
												<button
													type="button"
													key={option.type}
													aria-pressed={form.driver_type === option.type}
													onClick={() => onDriverTypeChange(option.type)}
													className={cn(
														"rounded-3xl border p-5 text-left transition",
														form.driver_type === option.type
															? "border-primary bg-primary/5 shadow-sm"
															: "border-border bg-background hover:border-primary/40 hover:bg-muted/20",
													)}
												>
													<div className="flex items-start gap-4">
														<div className="flex h-16 w-16 shrink-0 items-center justify-center rounded-2xl bg-white shadow-sm ring-1 ring-black/5">
															<img
																src={option.iconSrc}
																alt=""
																className={cn(
																	"w-auto object-contain",
																	option.type === "local"
																		? "max-h-8"
																		: "max-h-10",
																)}
															/>
														</div>
														<div className="min-w-0 flex-1">
															<div className="flex flex-wrap items-center gap-2">
																<p className="text-base font-semibold">
																	{option.title}
																</p>
																{form.driver_type === option.type ? (
																	<span className="rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
																		{t("policy_wizard_selected")}
																	</span>
																) : null}
															</div>
															<p className="mt-2 text-sm leading-6 text-muted-foreground">
																{option.description}
															</p>
														</div>
													</div>
												</button>
											))}
										</div>
									</div>
								) : createStep === 1 ? (
									<div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_280px]">
										<div className="space-y-4">
											{renderNameField(true)}
											{renderBasePathField()}
											{form.driver_type === "s3"
												? renderS3ConnectionFields(true)
												: null}
										</div>
										<div className="rounded-3xl border border-border/70 bg-muted/20 p-5">
											<div className="flex items-center gap-3">
												<div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-white shadow-sm ring-1 ring-black/5">
													<img
														src={currentStorageOption.iconSrc}
														alt=""
														className={cn(
															"w-auto object-contain",
															currentStorageOption.type === "local"
																? "max-h-7"
																: "max-h-9",
														)}
													/>
												</div>
												<div>
													<p className="text-sm font-medium">
														{currentStorageOption.title}
													</p>
													<p className="text-xs text-muted-foreground">
														{t("policy_wizard_driver_panel_title")}
													</p>
												</div>
											</div>
											<p className="mt-4 text-sm leading-6 text-muted-foreground">
												{currentStorageOption.description}
											</p>
											<p className="mt-4 text-xs leading-5 text-muted-foreground">
												{form.driver_type === "s3"
													? t("policy_wizard_s3_helper")
													: t("policy_wizard_local_helper")}
											</p>
										</div>
									</div>
								) : (
									<div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_300px]">
										<div className="space-y-4">
											{form.driver_type === "s3" ? (
												renderS3UploadStrategyField()
											) : (
												<div className="rounded-2xl border border-dashed border-border/80 bg-muted/20 p-4 text-sm text-muted-foreground">
													{t("policy_wizard_local_rules_helper")}
												</div>
											)}
											{renderLimitsFields()}
											{renderDefaultToggle()}
										</div>
										<div className="space-y-4 lg:sticky lg:top-0">
											<div className="rounded-3xl border border-border/70 bg-muted/20 p-5">
												<div className="flex items-center gap-3">
													<div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-white shadow-sm ring-1 ring-black/5">
														<img
															src={currentStorageOption.iconSrc}
															alt=""
															className={cn(
																"w-auto object-contain",
																currentStorageOption.type === "local"
																	? "max-h-7"
																	: "max-h-9",
															)}
														/>
													</div>
													<div>
														<p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
															{t("policy_wizard_summary_title")}
														</p>
														<h3 className="mt-1 text-base font-semibold">
															{form.name || t("new_policy")}
														</h3>
													</div>
												</div>
												<p className="mt-4 text-sm leading-6 text-muted-foreground">
													{t("policy_wizard_summary_desc")}
												</p>
												<div className="mt-4 overflow-hidden rounded-2xl border border-border/70 bg-background/85">
													<dl className="divide-y divide-border/70">
														{createSummaryItems.map((item) => (
															<div
																key={item.label}
																className="grid grid-cols-[96px_minmax(0,1fr)] items-start gap-3 px-4 py-3"
															>
																<dt className="pt-0.5 text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
																	{item.label}
																</dt>
																<dd className="min-w-0 break-all text-sm font-medium leading-5 text-foreground">
																	{item.value}
																</dd>
															</div>
														))}
													</dl>
												</div>
											</div>
										</div>
									</div>
								)}
							</div>
						</>
					) : (
						<>
							{renderNameField()}
							{renderBasePathField()}
							{form.driver_type === "s3" ? (
								<>
									{renderS3ConnectionFields()}
									{renderS3UploadStrategyField()}
								</>
							) : null}
							{renderLimitsFields()}
							{renderDefaultToggle()}
						</>
					)}

					<DialogFooter className="flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
						<div className="flex flex-wrap gap-2">
							<Button
								type="button"
								variant="outline"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={() => onOpenChange(false)}
								disabled={submitting}
							>
								{t("core:cancel")}
							</Button>
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

						<div className="flex flex-wrap justify-end gap-2">
							{isCreateMode ? (
								createStep === createLastStep ? (
									<>
										<TestConnectionButton
											onTest={onRunConnectionTest}
											disabled={submitting}
										/>
										<Button
											type="submit"
											className={ADMIN_CONTROL_HEIGHT_CLASS}
											disabled={submitting}
										>
											{t("core:create")}
										</Button>
									</>
								) : (
									<Button
										type="button"
										className={ADMIN_CONTROL_HEIGHT_CLASS}
										onClick={onCreateNext}
										disabled={submitting}
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
										disabled={submitting}
									/>
									<Button
										type="submit"
										className={ADMIN_CONTROL_HEIGHT_CLASS}
										disabled={submitting}
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
