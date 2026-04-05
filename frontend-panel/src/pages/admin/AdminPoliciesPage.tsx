import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import { StoragePolicyDialog } from "@/components/admin/StoragePolicyDialog";
import {
	buildCreatePolicyPayload,
	buildPolicyTestPayload,
	buildUpdatePolicyPayload,
	emptyForm,
	getEndpointValidationMessage,
	getPolicyForm,
	getS3ConnectionTestKey,
	hasConnectionFieldChanges,
	normalizePolicyForm,
	type PolicyFormData,
} from "@/components/admin/storagePolicyDialogShared";
import { AdminTableList } from "@/components/common/AdminTableList";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { AdminPageHeader } from "@/components/layout/AdminPageHeader";
import { AdminPageShell } from "@/components/layout/AdminPageShell";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import {
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { handleApiError } from "@/hooks/useApiError";
import { useApiList } from "@/hooks/useApiList";
import { useConfirmDialog } from "@/hooks/useConfirmDialog";
import {
	ADMIN_CONTROL_HEIGHT_CLASS,
	ADMIN_ICON_BUTTON_CLASS,
	ADMIN_TABLE_ACTIONS_WIDTH_CLASS,
} from "@/lib/constants";
import {
	buildOffsetPaginationSearchParams,
	parseOffsetSearchParam,
	parsePageSizeOption,
	parsePageSizeSearchParam,
} from "@/lib/pagination";
import { adminPolicyService } from "@/services/adminService";
import type { DriverType, StoragePolicy } from "@/types/api";

const POLICY_PAGE_SIZE_OPTIONS = [10, 20, 50] as const;
const DEFAULT_POLICY_PAGE_SIZE = 20 as const;
const CREATE_LAST_STEP = 2;
const PROTECTED_POLICY_ID = 1;
const INTERACTIVE_TABLE_ROW_CLASS =
	"cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50";

function getPolicyDriverBadgeClass(driverType: DriverType): string {
	return driverType === "s3"
		? "border-blue-500/60 bg-blue-500/10 text-blue-600 dark:text-blue-300"
		: "border-emerald-500/60 bg-emerald-500/10 text-emerald-600 dark:text-emerald-300";
}

const POLICY_TEXT_CELL_CONTENT_CLASS =
	"flex min-w-0 items-center rounded-lg bg-muted/10 px-3 py-3 text-left transition-colors duration-200";

const POLICY_BADGE_CELL_CONTENT_CLASS =
	"flex items-center rounded-lg bg-muted/20 px-3 py-3 text-left transition-colors duration-200";

export default function AdminPoliciesPage() {
	const { t } = useTranslation("admin");
	const [searchParams, setSearchParams] = useSearchParams();
	const [offset, setOffset] = useState(
		parseOffsetSearchParam(searchParams.get("offset")),
	);
	const [pageSize, setPageSize] = useState<
		(typeof POLICY_PAGE_SIZE_OPTIONS)[number]
	>(
		parsePageSizeSearchParam(
			searchParams.get("pageSize"),
			POLICY_PAGE_SIZE_OPTIONS,
			DEFAULT_POLICY_PAGE_SIZE,
		),
	);
	const {
		items: policies,
		setItems: setPolicies,
		total,
		loading,
		reload,
	} = useApiList(
		() => adminPolicyService.list({ limit: pageSize, offset }),
		[offset, pageSize],
	);
	const [dialogOpen, setDialogOpen] = useState(false);
	const [editingId, setEditingId] = useState<number | null>(null);
	const [editingPolicy, setEditingPolicy] = useState<StoragePolicy | null>(
		null,
	);
	const [form, setForm] = useState<PolicyFormData>(emptyForm);
	const [saveConfirmOpen, setSaveConfirmOpen] = useState(false);
	const [submitting, setSubmitting] = useState(false);
	const [validatedS3Key, setValidatedS3Key] = useState<string | null>(null);
	const [createStep, setCreateStep] = useState(0);
	const [createStepTouched, setCreateStepTouched] = useState(false);
	const endpointValidationMessage = getEndpointValidationMessage(form, t);
	const totalPages = Math.max(1, Math.ceil(total / pageSize));
	const currentPage = Math.floor(offset / pageSize) + 1;
	const prevPageDisabled = offset === 0;
	const nextPageDisabled = offset + pageSize >= total;
	const pageSizeOptions = POLICY_PAGE_SIZE_OPTIONS.map((size) => ({
		label: t("page_size_option", { count: size }),
		value: String(size),
	}));

	useEffect(() => {
		setSearchParams(
			buildOffsetPaginationSearchParams({
				offset,
				pageSize,
				defaultPageSize: DEFAULT_POLICY_PAGE_SIZE,
			}),
			{ replace: true },
		);
	}, [offset, pageSize, setSearchParams]);

	const handlePageSizeChange = (value: string | null) => {
		const next = parsePageSizeOption(value, POLICY_PAGE_SIZE_OPTIONS);
		if (next == null) return;
		setPageSize(next);
		setOffset(0);
	};

	const handleDelete = async (id: number) => {
		if (id === PROTECTED_POLICY_ID) return;
		try {
			await adminPolicyService.delete(id);
			if (policies.length === 1 && offset > 0) {
				setOffset(Math.max(0, offset - pageSize));
			} else {
				await reload();
			}
			toast.success(t("policy_deleted"));
		} catch (e) {
			handleApiError(e);
		}
	};

	const {
		confirmId: deleteId,
		requestConfirm,
		dialogProps,
	} = useConfirmDialog(handleDelete);
	const requestDeleteConfirm = (id: number) => {
		if (id === PROTECTED_POLICY_ID) return;
		requestConfirm(id);
	};

	const resetDialogState = () => {
		setSaveConfirmOpen(false);
		setValidatedS3Key(null);
		setCreateStep(0);
		setCreateStepTouched(false);
	};

	const openCreate = () => {
		setEditingId(null);
		setEditingPolicy(null);
		resetDialogState();
		setForm(emptyForm);
		setDialogOpen(true);
	};

	const openEdit = (policy: StoragePolicy) => {
		setEditingId(policy.id);
		setEditingPolicy(policy);
		resetDialogState();
		setForm(getPolicyForm(policy));
		setDialogOpen(true);
	};

	const handleDialogOpenChange = (open: boolean) => {
		setDialogOpen(open);
		if (!open) {
			resetDialogState();
		}
	};

	const setField = <K extends keyof PolicyFormData>(
		key: K,
		value: PolicyFormData[K],
	) => setForm((prev) => ({ ...prev, [key]: value }));

	const setDriverType = (driverType: DriverType) => {
		setValidatedS3Key(null);
		setCreateStepTouched(false);
		setForm((prev) =>
			driverType === "s3"
				? { ...prev, driver_type: driverType }
				: {
						...prev,
						driver_type: driverType,
						endpoint: "",
						bucket: "",
						access_key: "",
						secret_key: "",
						s3_upload_strategy: "relay_stream",
					},
		);
	};

	const syncNormalizedS3Form = () => {
		const normalizedForm = normalizePolicyForm(form);
		if (normalizedForm !== form) {
			setForm(normalizedForm);
		}
		return normalizedForm;
	};

	const runConnectionTest = async ({
		showSuccessToast = true,
		showFailureError = true,
	}: {
		showSuccessToast?: boolean;
		showFailureError?: boolean;
	} = {}) => {
		const currentForm = syncNormalizedS3Form();
		const shouldUseParamTest =
			editingId === null ||
			hasConnectionFieldChanges(currentForm, editingPolicy);

		try {
			if (shouldUseParamTest) {
				await adminPolicyService.testParams(
					buildPolicyTestPayload(currentForm),
				);
			} else {
				await adminPolicyService.testConnection(editingId);
			}

			if (currentForm.driver_type === "s3") {
				setValidatedS3Key(getS3ConnectionTestKey(currentForm));
			}
			if (showSuccessToast) {
				toast.success(t("connection_success"));
			}
			return true;
		} catch (e) {
			setValidatedS3Key(null);
			if (showFailureError) {
				handleApiError(e);
			}
			return false;
		}
	};

	const persistPolicy = async () => {
		try {
			const currentForm = syncNormalizedS3Form();
			if (editingId) {
				const updated = await adminPolicyService.update(
					editingId,
					buildUpdatePolicyPayload(currentForm),
				);
				setPolicies((prev) =>
					prev.map((policy) => (policy.id === editingId ? updated : policy)),
				);
				toast.success(t("policy_updated"));
			} else {
				await adminPolicyService.create(buildCreatePolicyPayload(currentForm));
				const nextTotal = total + 1;
				const nextLastOffset = Math.max(
					0,
					Math.floor((nextTotal - 1) / pageSize) * pageSize,
				);
				if (nextLastOffset !== offset) {
					setOffset(nextLastOffset);
				} else {
					await reload();
				}
				toast.success(t("policy_created"));
			}
			handleDialogOpenChange(false);
		} catch (e) {
			handleApiError(e);
		}
	};

	const shouldRunS3SaveTest = () => {
		if (form.driver_type !== "s3") {
			return false;
		}

		if (editingId !== null && !hasConnectionFieldChanges(form, editingPolicy)) {
			return false;
		}

		return validatedS3Key !== getS3ConnectionTestKey(form);
	};

	const submitPolicy = async (forceSave = false) => {
		if (submitting) {
			return;
		}

		setSubmitting(true);
		try {
			if (!forceSave && shouldRunS3SaveTest()) {
				const testPassed = await runConnectionTest({
					showSuccessToast: false,
					showFailureError: false,
				});
				if (!testPassed) {
					setSaveConfirmOpen(true);
					return;
				}
			}

			await persistPolicy();
		} finally {
			setSubmitting(false);
		}
	};

	const handleCreateBack = () => {
		setCreateStepTouched(false);
		setCreateStep((prev) => Math.max(0, prev - 1));
	};

	const handleCreateStepChange = (step: number) => {
		setCreateStepTouched(false);
		setCreateStep(Math.max(0, Math.min(CREATE_LAST_STEP, step)));
	};

	const handleCreateNext = () => {
		if (createStep >= CREATE_LAST_STEP) {
			return;
		}

		if (createStep === 0) {
			setCreateStep(1);
			return;
		}

		setCreateStepTouched(true);

		if (!form.name.trim()) {
			return;
		}

		if (form.driver_type === "s3" && !form.bucket.trim()) {
			return;
		}

		if (endpointValidationMessage) {
			return;
		}

		syncNormalizedS3Form();
		setCreateStepTouched(false);
		setCreateStep(CREATE_LAST_STEP);
	};

	const handleSubmit = () => {
		if (editingId === null && createStep < CREATE_LAST_STEP) {
			handleCreateNext();
			return;
		}
		void submitPolicy();
	};

	const deletePolicyName =
		deleteId !== null
			? (policies.find((policy) => policy.id === deleteId)?.name ?? "")
			: "";

	return (
		<AdminLayout>
			<AdminPageShell>
				<AdminPageHeader
					title={t("policies")}
					description={t("policies_intro")}
					actions={
						<>
							<Button
								size="sm"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={openCreate}
							>
								<Icon name="Plus" className="mr-1 h-4 w-4" />
								{t("new_policy")}
							</Button>
							<Button
								variant="outline"
								size="sm"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={() => void reload()}
								disabled={loading}
							>
								<Icon
									name={loading ? "Spinner" : "ArrowsClockwise"}
									className={`mr-1 h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`}
								/>
								{t("refresh")}
							</Button>
						</>
					}
				/>

				<AdminTableList
					loading={loading}
					items={policies}
					columns={7}
					rows={6}
					emptyTitle={t("no_policies")}
					emptyDescription={t("no_policies_desc")}
					headerRow={
						<TableHeader>
							<TableRow>
								<TableHead className="w-16">{t("id")}</TableHead>
								<TableHead>{t("core:name")}</TableHead>
								<TableHead>{t("driver_type")}</TableHead>
								<TableHead>{t("endpoint_path")}</TableHead>
								<TableHead>{t("bucket")}</TableHead>
								<TableHead className="w-20">{t("is_default")}</TableHead>
								<TableHead className={ADMIN_TABLE_ACTIONS_WIDTH_CLASS}>
									{t("core:actions")}
								</TableHead>
							</TableRow>
						</TableHeader>
					}
					renderRow={(policy) => (
						<TableRow
							key={policy.id}
							className={INTERACTIVE_TABLE_ROW_CLASS}
							onClick={() => openEdit(policy)}
							onKeyDown={(event) => {
								if (event.key === "Enter" || event.key === " ") {
									event.preventDefault();
									openEdit(policy);
								}
							}}
							tabIndex={0}
						>
							<TableCell>
								<div className={POLICY_TEXT_CELL_CONTENT_CLASS}>
									<span className="font-mono text-xs text-muted-foreground">
										{policy.id}
									</span>
								</div>
							</TableCell>
							<TableCell>
								<div className={POLICY_TEXT_CELL_CONTENT_CLASS}>
									<div className="min-w-0">
										<div className="truncate font-medium text-foreground">
											{policy.name}
										</div>
									</div>
								</div>
							</TableCell>
							<TableCell>
								<div className={POLICY_BADGE_CELL_CONTENT_CLASS}>
									<Badge
										variant="outline"
										className={getPolicyDriverBadgeClass(policy.driver_type)}
									>
										{policy.driver_type === "local" ? "Local" : "S3"}
									</Badge>
								</div>
							</TableCell>
							<TableCell>
								<div className={POLICY_TEXT_CELL_CONTENT_CLASS}>
									<span className="truncate text-xs font-mono text-muted-foreground">
										{policy.driver_type === "local"
											? policy.base_path || "./data"
											: policy.endpoint}
									</span>
								</div>
							</TableCell>
							<TableCell>
								<div className={POLICY_TEXT_CELL_CONTENT_CLASS}>
									<span className="truncate text-xs text-muted-foreground">
										{policy.bucket || "-"}
									</span>
								</div>
							</TableCell>
							<TableCell>
								<div className={POLICY_BADGE_CELL_CONTENT_CLASS}>
									{policy.is_default ? (
										<Badge className="bg-blue-100 border-blue-300 text-blue-700 dark:border-blue-700 dark:bg-blue-900 dark:text-blue-300">
											{t("is_default")}
										</Badge>
									) : (
										<span className="text-xs text-muted-foreground">-</span>
									)}
								</div>
							</TableCell>
							<TableCell
								onClick={(event) => event.stopPropagation()}
								onKeyDown={(event) => event.stopPropagation()}
							>
								<div className="flex justify-end">
									<TooltipProvider>
										<Tooltip>
											<TooltipTrigger>
												<div>
													<Button
														variant="ghost"
														size="icon"
														className={`${ADMIN_ICON_BUTTON_CLASS} text-destructive`}
														onClick={() => requestDeleteConfirm(policy.id)}
														aria-label={t("delete_policy")}
														title={t("delete_policy")}
														disabled={policy.id === PROTECTED_POLICY_ID}
													>
														<Icon name="Trash" className="h-3.5 w-3.5" />
													</Button>
												</div>
											</TooltipTrigger>
											{policy.id === PROTECTED_POLICY_ID ? (
												<TooltipContent>
													{t("initial_policy_delete_blocked")}
												</TooltipContent>
											) : null}
										</Tooltip>
									</TooltipProvider>
								</div>
							</TableCell>
						</TableRow>
					)}
				/>

				{total > 0 ? (
					<div className="flex items-center justify-between gap-3 px-4 pb-4 text-sm text-muted-foreground md:px-6">
						<div className="flex items-center gap-3">
							<span>
								{t("entries_page", {
									total,
									current: currentPage,
									pages: totalPages,
								})}
							</span>
							<Select
								value={String(pageSize)}
								onValueChange={handlePageSizeChange}
							>
								<SelectTrigger
									className={`${ADMIN_CONTROL_HEIGHT_CLASS} w-[120px]`}
								>
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									{pageSizeOptions.map((option) => (
										<SelectItem key={option.value} value={option.value}>
											{option.label}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
						</div>
						<TooltipProvider>
							<div className="flex items-center gap-2">
								<Tooltip>
									<TooltipTrigger
										render={
											<Button
												variant="outline"
												size="sm"
												disabled={prevPageDisabled}
												onClick={() =>
													setOffset(Math.max(0, offset - pageSize))
												}
											/>
										}
									>
										<Icon name="CaretLeft" className="h-4 w-4" />
									</TooltipTrigger>
									{prevPageDisabled ? (
										<TooltipContent>
											{t("pagination_prev_disabled")}
										</TooltipContent>
									) : null}
								</Tooltip>
								<Tooltip>
									<TooltipTrigger
										render={
											<Button
												variant="outline"
												size="sm"
												disabled={nextPageDisabled}
												onClick={() => setOffset(offset + pageSize)}
											/>
										}
									>
										<Icon name="CaretRight" className="h-4 w-4" />
									</TooltipTrigger>
									{nextPageDisabled ? (
										<TooltipContent>
											{t("pagination_next_disabled")}
										</TooltipContent>
									) : null}
								</Tooltip>
							</div>
						</TooltipProvider>
					</div>
				) : null}

				<ConfirmDialog
					{...dialogProps}
					title={`${t("delete_policy")} "${deletePolicyName}"?`}
					description={t("delete_policy_desc")}
					confirmLabel={t("core:delete")}
					variant="destructive"
				/>

				<ConfirmDialog
					open={saveConfirmOpen}
					onOpenChange={setSaveConfirmOpen}
					title={t("connection_test_failed")}
					description={t("policy_test_failed_confirm_desc")}
					confirmLabel={t("save_anyway")}
					onConfirm={() => {
						setSaveConfirmOpen(false);
						void submitPolicy(true);
					}}
				/>

				<StoragePolicyDialog
					open={dialogOpen}
					mode={editingId === null ? "create" : "edit"}
					form={form}
					submitting={submitting}
					createStep={createStep}
					createStepTouched={createStepTouched}
					endpointValidationMessage={endpointValidationMessage}
					onOpenChange={handleDialogOpenChange}
					onSubmit={handleSubmit}
					onRunConnectionTest={() => runConnectionTest()}
					onFieldChange={setField}
					onDriverTypeChange={setDriverType}
					onCreateBack={handleCreateBack}
					onCreateStepChange={handleCreateStepChange}
					onCreateNext={handleCreateNext}
					onSyncNormalizedS3Form={syncNormalizedS3Form}
				/>
			</AdminPageShell>
		</AdminLayout>
	);
}
