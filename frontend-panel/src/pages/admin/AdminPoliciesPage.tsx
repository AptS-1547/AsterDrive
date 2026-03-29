import type { FormEvent } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { AdminTableList } from "@/components/common/AdminTableList";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { AdminPageHeader } from "@/components/layout/AdminPageHeader";
import { AdminPageShell } from "@/components/layout/AdminPageShell";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
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
import {
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { useApiList } from "@/hooks/useApiList";
import { useConfirmDialog } from "@/hooks/useConfirmDialog";
import { ADMIN_ICON_BUTTON_CLASS } from "@/lib/constants";
import { adminPolicyService } from "@/services/adminService";
import type { DriverType, StoragePolicy } from "@/types/api";

type S3UploadStrategy = "proxy_tempfile" | "relay_stream" | "presigned";

interface PolicyFormData {
	name: string;
	driver_type: DriverType;
	endpoint: string;
	bucket: string;
	access_key: string;
	secret_key: string;
	base_path: string;
	max_file_size: string;
	chunk_size: string;
	is_default: boolean;
	s3_upload_strategy: S3UploadStrategy;
}

interface PolicyOptions {
	presigned_upload?: boolean;
	s3_upload_strategy?: S3UploadStrategy;
}

function isS3UploadStrategy(value: unknown): value is S3UploadStrategy {
	return (
		value === "proxy_tempfile" ||
		value === "relay_stream" ||
		value === "presigned"
	);
}

function parsePolicyOptions(options: string): PolicyOptions {
	try {
		const parsed = JSON.parse(options) as {
			presigned_upload?: unknown;
			s3_upload_strategy?: unknown;
		};
		return {
			presigned_upload:
				typeof parsed.presigned_upload === "boolean"
					? parsed.presigned_upload
					: undefined,
			s3_upload_strategy: isS3UploadStrategy(parsed.s3_upload_strategy)
				? parsed.s3_upload_strategy
				: undefined,
		};
	} catch {
		return {};
	}
}

function getEffectiveS3UploadStrategy(
	options: PolicyOptions,
): S3UploadStrategy {
	if (options.s3_upload_strategy) {
		return options.s3_upload_strategy;
	}
	return options.presigned_upload ? "presigned" : "proxy_tempfile";
}

function buildPolicyTestPayload(form: PolicyFormData) {
	return {
		driver_type: form.driver_type,
		endpoint: form.endpoint || undefined,
		bucket: form.bucket || undefined,
		access_key: form.access_key || undefined,
		secret_key: form.secret_key || undefined,
		base_path: form.base_path || undefined,
	};
}

function hasConnectionFieldChanges(
	form: PolicyFormData,
	editingPolicy: StoragePolicy | null,
) {
	if (!editingPolicy) {
		return true;
	}

	if (form.driver_type === "s3") {
		return (
			form.endpoint !== editingPolicy.endpoint ||
			form.bucket !== editingPolicy.bucket ||
			form.base_path !== editingPolicy.base_path ||
			form.access_key !== "" ||
			form.secret_key !== ""
		);
	}

	return form.base_path !== editingPolicy.base_path;
}

function getS3ConnectionTestKey(form: PolicyFormData) {
	return JSON.stringify({
		driver_type: form.driver_type,
		endpoint: form.endpoint,
		bucket: form.bucket,
		access_key: form.access_key,
		secret_key: form.secret_key,
		base_path: form.base_path,
	});
}

const emptyForm: PolicyFormData = {
	name: "",
	driver_type: "local",
	endpoint: "",
	bucket: "",
	access_key: "",
	secret_key: "",
	base_path: "",
	max_file_size: "",
	chunk_size: "5",
	is_default: false,
	s3_upload_strategy: "proxy_tempfile",
};

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

export default function AdminPoliciesPage() {
	const { t } = useTranslation("admin");
	const {
		items: policies,
		setItems: setPolicies,
		loading,
	} = useApiList(() => adminPolicyService.list({ limit: 100, offset: 0 }));
	const [dialogOpen, setDialogOpen] = useState(false);
	const [editingId, setEditingId] = useState<number | null>(null);
	const [editingPolicy, setEditingPolicy] = useState<StoragePolicy | null>(
		null,
	);
	const [form, setForm] = useState<PolicyFormData>(emptyForm);
	const [saveConfirmOpen, setSaveConfirmOpen] = useState(false);
	const [submitting, setSubmitting] = useState(false);
	const [validatedS3Key, setValidatedS3Key] = useState<string | null>(null);

	const handleDelete = async (id: number) => {
		try {
			await adminPolicyService.delete(id);
			setPolicies((prev) => prev.filter((p) => p.id !== id));
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

	const openCreate = () => {
		setEditingId(null);
		setEditingPolicy(null);
		setValidatedS3Key(null);
		setSaveConfirmOpen(false);
		setForm(emptyForm);
		setDialogOpen(true);
	};

	const openEdit = (p: StoragePolicy) => {
		setEditingId(p.id);
		setEditingPolicy(p);
		setValidatedS3Key(null);
		setSaveConfirmOpen(false);
		const opts = parsePolicyOptions(p.options);
		setForm({
			name: p.name,
			driver_type: p.driver_type,
			endpoint: p.endpoint,
			bucket: p.bucket,
			access_key: "",
			secret_key: "",
			base_path: p.base_path,
			max_file_size: p.max_file_size != null ? String(p.max_file_size) : "",
			chunk_size:
				p.chunk_size != null
					? String(Math.round(p.chunk_size / 1024 / 1024))
					: "5",
			is_default: p.is_default,
			s3_upload_strategy: getEffectiveS3UploadStrategy(opts),
		});
		setDialogOpen(true);
	};

	const handleDialogOpenChange = (open: boolean) => {
		setDialogOpen(open);
		if (!open) {
			setSaveConfirmOpen(false);
			setValidatedS3Key(null);
		}
	};

	const runConnectionTest = async ({
		showSuccessToast = true,
		showFailureError = true,
	}: {
		showSuccessToast?: boolean;
		showFailureError?: boolean;
	} = {}) => {
		const shouldUseParamTest =
			editingId === null || hasConnectionFieldChanges(form, editingPolicy);

		try {
			if (shouldUseParamTest) {
				await adminPolicyService.testParams(buildPolicyTestPayload(form));
			} else {
				await adminPolicyService.testConnection(editingId);
			}

			if (form.driver_type === "s3") {
				setValidatedS3Key(getS3ConnectionTestKey(form));
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
			const options = JSON.stringify({
				s3_upload_strategy: form.s3_upload_strategy,
			});
			if (editingId) {
				const payload: Record<string, unknown> = {
					name: form.name,
					endpoint: form.endpoint,
					bucket: form.bucket,
					base_path: form.base_path,
					max_file_size: form.max_file_size
						? Number(form.max_file_size)
						: undefined,
					chunk_size: form.chunk_size
						? Number(form.chunk_size) * 1024 * 1024
						: 0,
					is_default: form.is_default,
					options,
				};
				// Only send credentials if user typed new values
				if (form.access_key) payload.access_key = form.access_key;
				if (form.secret_key) payload.secret_key = form.secret_key;
				const updated = await adminPolicyService.update(editingId, payload);
				setPolicies((prev) =>
					prev.map((p) => (p.id === editingId ? updated : p)),
				);
				toast.success(t("policy_updated"));
			} else {
				const payload = {
					name: form.name,
					driver_type: form.driver_type,
					endpoint: form.endpoint,
					bucket: form.bucket,
					access_key: form.access_key,
					secret_key: form.secret_key,
					base_path: form.base_path,
					max_file_size: form.max_file_size
						? Number(form.max_file_size)
						: undefined,
					chunk_size: form.chunk_size
						? Number(form.chunk_size) * 1024 * 1024
						: 0,
					is_default: form.is_default,
					options,
				};
				const created = await adminPolicyService.create(payload);
				setPolicies((prev) => [...prev, created]);
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

	const handleSubmit = (e: FormEvent) => {
		e.preventDefault();
		void submitPolicy();
	};

	const setField = <K extends keyof PolicyFormData>(
		key: K,
		value: PolicyFormData[K],
	) => setForm((prev) => ({ ...prev, [key]: value }));

	const deletePolicyName =
		deleteId !== null
			? (policies.find((p) => p.id === deleteId)?.name ?? "")
			: "";

	return (
		<AdminLayout>
			<AdminPageShell>
				<AdminPageHeader
					title={t("policies")}
					description={t("policies_intro")}
					actions={
						<Button size="sm" onClick={openCreate}>
							<Icon name="Plus" className="mr-1 h-4 w-4" />
							{t("new_policy")}
						</Button>
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
								<TableHead className="w-24">{t("core:actions")}</TableHead>
							</TableRow>
						</TableHeader>
					}
					renderRow={(p) => (
						<TableRow key={p.id}>
							<TableCell className="font-mono text-xs">{p.id}</TableCell>
							<TableCell className="font-medium">{p.name}</TableCell>
							<TableCell>
								<Badge variant="outline">
									{p.driver_type === "local" ? "Local" : "S3"}
								</Badge>
							</TableCell>
							<TableCell className="text-muted-foreground text-xs font-mono">
								{p.driver_type === "local"
									? p.base_path || "./data"
									: p.endpoint}
							</TableCell>
							<TableCell className="text-muted-foreground text-xs">
								{p.bucket || "-"}
							</TableCell>
							<TableCell>
								{p.is_default && (
									<Badge className="bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300 border-blue-300 dark:border-blue-700">
										{t("is_default")}
									</Badge>
								)}
							</TableCell>
							<TableCell>
								<div className="flex items-center gap-1">
									<Button
										variant="ghost"
										size="icon"
										className={ADMIN_ICON_BUTTON_CLASS}
										onClick={() => openEdit(p)}
									>
										<Icon name="PencilSimple" className="h-3.5 w-3.5" />
									</Button>
									<Button
										variant="ghost"
										size="icon"
										className={`${ADMIN_ICON_BUTTON_CLASS} text-destructive`}
										onClick={() => requestConfirm(p.id)}
									>
										<Icon name="Trash" className="h-3.5 w-3.5" />
									</Button>
								</div>
							</TableCell>
						</TableRow>
					)}
				/>

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

				<Dialog open={dialogOpen} onOpenChange={handleDialogOpenChange}>
					<DialogContent className="max-w-lg">
						<DialogHeader>
							<DialogTitle>
								{editingId ? t("edit_policy") : t("create_policy")}
							</DialogTitle>
						</DialogHeader>
						<form onSubmit={handleSubmit} className="space-y-4">
							<div className="space-y-2">
								<Label htmlFor="name">{t("core:name")}</Label>
								<Input
									id="name"
									value={form.name}
									onChange={(e) => setField("name", e.target.value)}
									required
								/>
							</div>

							{!editingId && (
								<div className="space-y-2">
									<Label>{t("driver_type")}</Label>
									<Select
										value={form.driver_type}
										onValueChange={(v) =>
											setField("driver_type", v as DriverType)
										}
									>
										<SelectTrigger>
											<SelectValue />
										</SelectTrigger>
										<SelectContent>
											<SelectItem value="local">
												{t("driver_type_local")}
											</SelectItem>
											<SelectItem value="s3">{t("driver_type_s3")}</SelectItem>
										</SelectContent>
									</Select>
								</div>
							)}

							<div className="space-y-2">
								<Label htmlFor="base_path">{t("base_path")}</Label>
								<Input
									id="base_path"
									value={form.base_path}
									onChange={(e) => setField("base_path", e.target.value)}
									placeholder={
										form.driver_type === "local" ? "./data" : "prefix/path"
									}
								/>
							</div>

							{form.driver_type === "s3" && (
								<>
									<div className="space-y-2">
										<Label htmlFor="endpoint">{t("endpoint")}</Label>
										<Input
											id="endpoint"
											value={form.endpoint}
											onChange={(e) => setField("endpoint", e.target.value)}
											placeholder="https://s3.amazonaws.com"
										/>
									</div>
									<div className="space-y-2">
										<Label htmlFor="bucket">{t("bucket")}</Label>
										<Input
											id="bucket"
											value={form.bucket}
											onChange={(e) => setField("bucket", e.target.value)}
											required
										/>
									</div>
									<div className="grid grid-cols-2 gap-4">
										<div className="space-y-2">
											<Label htmlFor="access_key">{t("access_key")}</Label>
											<Input
												id="access_key"
												value={form.access_key}
												onChange={(e) => setField("access_key", e.target.value)}
											/>
										</div>
										<div className="space-y-2">
											<Label htmlFor="secret_key">{t("secret_key")}</Label>
											<Input
												id="secret_key"
												type="password"
												value={form.secret_key}
												onChange={(e) => setField("secret_key", e.target.value)}
											/>
										</div>
									</div>
									<div className="space-y-2 pt-1">
										<Label htmlFor="s3_upload_strategy">
											{t("s3_upload_strategy")}
										</Label>
										<Select
											value={form.s3_upload_strategy}
											onValueChange={(value) =>
												setField(
													"s3_upload_strategy",
													value as S3UploadStrategy,
												)
											}
										>
											<SelectTrigger id="s3_upload_strategy">
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
								</>
							)}

							<div className="space-y-2">
								<Label htmlFor="max_file_size">
									{t("max_file_size")} (bytes)
								</Label>
								<Input
									id="max_file_size"
									type="number"
									value={form.max_file_size}
									onChange={(e) => setField("max_file_size", e.target.value)}
									placeholder={`0 = ${t("core:unlimited").toLowerCase()}`}
								/>
							</div>

							<div className="space-y-2">
								<Label htmlFor="chunk_size">{t("chunk_size")}</Label>
								<Input
									id="chunk_size"
									type="number"
									value={form.chunk_size}
									onChange={(e) => setField("chunk_size", e.target.value)}
									placeholder="5 = 5MB, 0 = single upload only"
								/>
								<p className="text-xs text-muted-foreground">
									{t("chunk_size_desc")}
								</p>
							</div>

							<div className="flex items-center gap-2">
								<Switch
									id="is_default"
									checked={form.is_default}
									onCheckedChange={(v) => setField("is_default", v)}
								/>
								<Label htmlFor="is_default">{t("set_as_default")}</Label>
							</div>

							<div className="flex gap-2">
								<TestConnectionButton
									onTest={() => runConnectionTest()}
									disabled={submitting}
								/>
								<Button type="submit" className="flex-1" disabled={submitting}>
									{editingId ? t("save_changes") : t("core:create")}
								</Button>
							</div>
						</form>
					</DialogContent>
				</Dialog>
			</AdminPageShell>
		</AdminLayout>
	);
}
