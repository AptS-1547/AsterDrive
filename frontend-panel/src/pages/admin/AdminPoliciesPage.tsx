import type { FormEvent } from "react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AdminLayout } from "@/components/layout/AdminLayout";
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
import { ScrollArea } from "@/components/ui/scroll-area";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { adminPolicyService } from "@/services/adminService";
import type { DriverType, StoragePolicy } from "@/types/api";

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
	presigned_upload: boolean;
}

interface PolicyOptions {
	presigned_upload?: boolean;
}

function parsePolicyOptions(options: string): PolicyOptions {
	try {
		return JSON.parse(options);
	} catch {
		return {};
	}
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
	presigned_upload: false,
};

function TestConnectionButton({
	form,
	editingId,
}: {
	form: PolicyFormData;
	editingId: number | null;
}) {
	const { t } = useTranslation("admin");
	const [testing, setTesting] = useState(false);
	const [result, setResult] = useState<boolean | null>(null);

	const handleTest = async () => {
		setTesting(true);
		setResult(null);
		try {
			if (editingId) {
				await adminPolicyService.testConnection(editingId);
			} else {
				await adminPolicyService.testParams({
					driver_type: form.driver_type,
					endpoint: form.endpoint || undefined,
					bucket: form.bucket || undefined,
					access_key: form.access_key || undefined,
					secret_key: form.secret_key || undefined,
					base_path: form.base_path || undefined,
				});
			}
			setResult(true);
			toast.success(t("connection_success"));
		} catch (e) {
			setResult(false);
			handleApiError(e);
		} finally {
			setTesting(false);
		}
	};

	return (
		<Button
			type="button"
			variant="outline"
			disabled={testing}
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
	const [policies, setPolicies] = useState<StoragePolicy[]>([]);
	const [loading, setLoading] = useState(true);
	const [dialogOpen, setDialogOpen] = useState(false);
	const [editingId, setEditingId] = useState<number | null>(null);
	const [form, setForm] = useState<PolicyFormData>(emptyForm);
	const [deleteId, setDeleteId] = useState<number | null>(null);

	const load = useCallback(async () => {
		try {
			setLoading(true);
			const data = await adminPolicyService.list();
			setPolicies(data);
		} catch (e) {
			handleApiError(e);
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		load();
	}, [load]);

	const openCreate = () => {
		setEditingId(null);
		setForm(emptyForm);
		setDialogOpen(true);
	};

	const openEdit = (p: StoragePolicy) => {
		setEditingId(p.id);
		const opts = parsePolicyOptions(p.options);
		setForm({
			name: p.name,
			driver_type: p.driver_type,
			endpoint: p.endpoint,
			bucket: p.bucket,
			access_key: "",
			secret_key: "",
			base_path: p.base_path,
			max_file_size: p.max_file_size ? String(p.max_file_size) : "",
			chunk_size: p.chunk_size
				? String(Math.round(p.chunk_size / 1024 / 1024))
				: "5",
			is_default: p.is_default,
			presigned_upload: opts.presigned_upload ?? false,
		});
		setDialogOpen(true);
	};

	const handleSubmit = async (e: FormEvent) => {
		e.preventDefault();
		try {
			const options = JSON.stringify({
				presigned_upload: form.presigned_upload,
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
					...form,
					max_file_size: form.max_file_size
						? Number(form.max_file_size)
						: undefined,
					chunk_size: form.chunk_size
						? Number(form.chunk_size) * 1024 * 1024
						: 0,
					options,
				};
				const created = await adminPolicyService.create(payload);
				setPolicies((prev) => [...prev, created]);
				toast.success(t("policy_created"));
			}
			setDialogOpen(false);
		} catch (e) {
			handleApiError(e);
		}
	};

	const handleDelete = async (id: number) => {
		try {
			await adminPolicyService.delete(id);
			setPolicies((prev) => prev.filter((p) => p.id !== id));
			toast.success(t("policy_deleted"));
		} catch (e) {
			handleApiError(e);
		}
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
			<div className="p-6 space-y-4">
				<div className="flex items-center justify-between">
					<h2 className="text-lg font-semibold">{t("policies")}</h2>
					<Button size="sm" onClick={openCreate}>
						<Icon name="Plus" className="h-4 w-4 mr-1" />
						{t("new_policy")}
					</Button>
				</div>

				{loading ? (
					<SkeletonTable columns={7} rows={6} />
				) : policies.length === 0 ? (
					<EmptyState
						title={t("no_policies")}
						description={t("no_policies_desc")}
					/>
				) : (
					<ScrollArea className="flex-1">
						<Table>
							<TableHeader>
								<TableRow>
									<TableHead className="w-16">{t("id")}</TableHead>
									<TableHead>{t("common:name")}</TableHead>
									<TableHead>{t("driver_type")}</TableHead>
									<TableHead>{t("endpoint_path")}</TableHead>
									<TableHead>{t("bucket")}</TableHead>
									<TableHead className="w-20">{t("is_default")}</TableHead>
									<TableHead className="w-24">{t("common:actions")}</TableHead>
								</TableRow>
							</TableHeader>
							<TableBody>
								{policies.map((p) => (
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
													className="h-8 w-8"
													onClick={() => openEdit(p)}
												>
													<Icon name="PencilSimple" className="h-3.5 w-3.5" />
												</Button>
												<Button
													variant="ghost"
													size="icon"
													className="h-8 w-8 text-destructive"
													onClick={() => setDeleteId(p.id)}
												>
													<Icon name="Trash" className="h-3.5 w-3.5" />
												</Button>
											</div>
										</TableCell>
									</TableRow>
								))}
							</TableBody>
						</Table>
					</ScrollArea>
				)}

				<ConfirmDialog
					open={deleteId !== null}
					onOpenChange={(open) => {
						if (!open) setDeleteId(null);
					}}
					title={`${t("delete_policy")} "${deletePolicyName}"?`}
					description={t("delete_policy_desc")}
					confirmLabel={t("common:delete")}
					onConfirm={() => {
						if (deleteId !== null) handleDelete(deleteId);
						setDeleteId(null);
					}}
					variant="destructive"
				/>

				<Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
					<DialogContent className="max-w-lg">
						<DialogHeader>
							<DialogTitle>
								{editingId ? t("edit_policy") : t("create_policy")}
							</DialogTitle>
						</DialogHeader>
						<form onSubmit={handleSubmit} className="space-y-4">
							<div className="space-y-2">
								<Label htmlFor="name">{t("common:name")}</Label>
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
											<SelectItem value="local">Local</SelectItem>
											<SelectItem value="s3">S3</SelectItem>
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
											<Label htmlFor="access_key">Access Key</Label>
											<Input
												id="access_key"
												value={form.access_key}
												onChange={(e) => setField("access_key", e.target.value)}
											/>
										</div>
										<div className="space-y-2">
											<Label htmlFor="secret_key">Secret Key</Label>
											<Input
												id="secret_key"
												type="password"
												value={form.secret_key}
												onChange={(e) => setField("secret_key", e.target.value)}
											/>
										</div>
									</div>
									<div className="flex items-center gap-2 pt-1">
										<Switch
											id="presigned_upload"
											checked={form.presigned_upload}
											onCheckedChange={(v) => setField("presigned_upload", v)}
										/>
										<div>
											<Label htmlFor="presigned_upload">
												{t("presigned_upload")}
											</Label>
											<p className="text-xs text-muted-foreground">
												{t("presigned_upload_desc")}
											</p>
										</div>
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
									placeholder={`0 = ${t("common:unlimited").toLowerCase()}`}
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
								<TestConnectionButton form={form} editingId={editingId} />
								<Button type="submit" className="flex-1">
									{editingId ? t("save_changes") : t("common:create")}
								</Button>
							</div>
						</form>
					</DialogContent>
				</Dialog>
			</div>
		</AdminLayout>
	);
}
