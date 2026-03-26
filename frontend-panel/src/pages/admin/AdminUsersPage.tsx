import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
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
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
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
import { formatBytes } from "@/lib/format";
import {
	adminPolicyService,
	adminUserPolicyService,
	adminUserService,
} from "@/services/adminService";
import type {
	StoragePolicy,
	UserInfo,
	UserRole,
	UserStatus,
	UserStoragePolicy,
} from "@/types/api";

function QuotaCell({
	user,
	onUpdate,
}: {
	user: UserInfo;
	onUpdate: (id: number, quota: number) => void;
}) {
	const { t } = useTranslation();
	const [editing, setEditing] = useState(false);
	const [value, setValue] = useState("");

	const quota = user.storage_quota ?? 0;
	const used = user.storage_used ?? 0;
	const pct = quota > 0 ? Math.min((used / quota) * 100, 100) : 0;

	const startEdit = () => {
		setValue(quota > 0 ? String(Math.round(quota / 1024 / 1024)) : "0");
		setEditing(true);
	};

	const confirm = () => {
		const mb = Number.parseInt(value, 10);
		const newQuota = Number.isNaN(mb) || mb <= 0 ? 0 : mb * 1024 * 1024;
		onUpdate(user.id, newQuota);
		setEditing(false);
	};

	if (editing) {
		return (
			<div className="flex items-center gap-1">
				<Input
					className="h-7 w-20 text-xs"
					value={value}
					onChange={(e) => setValue(e.target.value)}
					onKeyDown={(e) => {
						if (e.key === "Enter") confirm();
						if (e.key === "Escape") setEditing(false);
					}}
					onBlur={confirm}
					autoFocus
					placeholder="MB"
				/>
				<span className="text-xs text-muted-foreground">MB</span>
			</div>
		);
	}

	return (
		<button
			type="button"
			className="text-left w-full group cursor-pointer"
			onClick={startEdit}
		>
			<div className="text-xs">
				{formatBytes(used)}
				{quota > 0 ? ` / ${formatBytes(quota)}` : ` / ${t("unlimited")}`}
			</div>
			{quota > 0 && <Progress value={pct} className="h-1.5 mt-1" />}
		</button>
	);
}

function UserPolicyDialog({
	userId,
	open,
	onOpenChange,
}: {
	userId: number;
	open: boolean;
	onOpenChange: (open: boolean) => void;
}) {
	const { t } = useTranslation("admin");
	const [assignments, setAssignments] = useState<UserStoragePolicy[]>([]);
	const [policies, setPolicies] = useState<StoragePolicy[]>([]);
	const [loading, setLoading] = useState(true);
	const [addPolicyId, setAddPolicyId] = useState<number | null>(null);
	const [addQuota, setAddQuota] = useState("");
	const [addDefault, setAddDefault] = useState(false);

	const load = useCallback(async () => {
		try {
			setLoading(true);
			const [a, p] = await Promise.all([
				adminUserPolicyService.list(userId),
				adminPolicyService.list(),
			]);
			setAssignments(a);
			setPolicies(p);
		} catch (e) {
			handleApiError(e);
		} finally {
			setLoading(false);
		}
	}, [userId]);

	useEffect(() => {
		if (open) load();
	}, [open, load]);

	const policyName = (policyId: number) =>
		policies.find((p) => p.id === policyId)?.name ?? `#${policyId}`;

	const handleAssign = async () => {
		if (!addPolicyId) return;
		try {
			const mb = Number.parseInt(addQuota, 10);
			const quotaBytes = Number.isNaN(mb) || mb <= 0 ? 0 : mb * 1024 * 1024;
			const created = await adminUserPolicyService.assign(userId, {
				policy_id: addPolicyId,
				is_default: addDefault,
				quota_bytes: quotaBytes,
			});
			setAssignments((prev) =>
				addDefault
					? [...prev.map((a) => ({ ...a, is_default: false })), created]
					: [...prev, created],
			);
			setAddPolicyId(null);
			setAddQuota("");
			setAddDefault(false);
			toast.success(t("policy_assigned"));
		} catch (e) {
			handleApiError(e);
		}
	};

	const handleToggleDefault = async (a: UserStoragePolicy) => {
		try {
			const updated = await adminUserPolicyService.update(userId, a.id, {
				is_default: !a.is_default,
			});
			setAssignments((prev) =>
				prev.map((item) =>
					item.id === updated.id
						? updated
						: !a.is_default
							? { ...item, is_default: false }
							: item,
				),
			);
		} catch (e) {
			handleApiError(e);
		}
	};

	const handleRemove = async (id: number) => {
		try {
			await adminUserPolicyService.remove(userId, id);
			setAssignments((prev) => prev.filter((a) => a.id !== id));
			toast.success(t("assignment_removed"));
		} catch (e) {
			handleApiError(e);
		}
	};

	// Policies not yet assigned
	const availablePolicies = policies.filter(
		(p) => !assignments.some((a) => a.policy_id === p.id),
	);

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="max-w-lg">
				<DialogHeader>
					<DialogTitle>{t("storage_policy_assignments")}</DialogTitle>
				</DialogHeader>

				{loading ? (
					<SkeletonTable columns={4} rows={5} />
				) : (
					<div className="space-y-4">
						{/* Existing assignments */}
						{assignments.length === 0 ? (
							<p className="text-sm text-muted-foreground">
								{t("no_policies_assigned")}
							</p>
						) : (
							<div className="space-y-2">
								{assignments.map((a) => (
									<div
										key={a.id}
										className="flex items-center justify-between p-3 border rounded-lg"
									>
										<div>
											<div className="text-sm font-medium">
												{policyName(a.policy_id)}
											</div>
											<div className="text-xs text-muted-foreground">
												{t("quota")}:{" "}
												{a.quota_bytes > 0
													? formatBytes(a.quota_bytes)
													: t("common:unlimited")}
											</div>
										</div>
										<div className="flex items-center gap-2">
											{a.is_default && (
												<Badge className="bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300 border-blue-300 dark:border-blue-700">
													{t("is_default")}
												</Badge>
											)}
											<Button
												variant="ghost"
												size="sm"
												onClick={() => handleToggleDefault(a)}
											>
												{a.is_default ? t("unset_default") : t("set_default")}
											</Button>
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8 text-destructive"
												onClick={() => handleRemove(a.id)}
											>
												<Icon name="Trash" className="h-3.5 w-3.5" />
											</Button>
										</div>
									</div>
								))}
							</div>
						)}

						{/* Add new assignment */}
						{availablePolicies.length > 0 && (
							<div className="border-t pt-4 space-y-3">
								<Label className="text-sm font-medium">
									<Icon name="Plus" className="h-3.5 w-3.5 inline mr-1" />
									{t("assign_policy")}
								</Label>
								<Select
									value={addPolicyId != null ? String(addPolicyId) : ""}
									onValueChange={(v) => setAddPolicyId(v ? Number(v) : null)}
								>
									<SelectTrigger>
										<SelectValue placeholder={t("select_policy")} />
									</SelectTrigger>
									<SelectContent>
										{availablePolicies.map((p) => (
											<SelectItem key={p.id} value={String(p.id)}>
												{p.name} ({p.driver_type})
											</SelectItem>
										))}
									</SelectContent>
								</Select>
								<div className="flex items-center gap-3">
									<div className="flex-1">
										<Input
											placeholder={`${t("quota")} (MB, 0=${t("common:unlimited").toLowerCase()})`}
											value={addQuota}
											onChange={(e) => setAddQuota(e.target.value)}
										/>
									</div>
									<div className="flex items-center gap-2">
										<Switch
											checked={addDefault}
											onCheckedChange={setAddDefault}
										/>
										<span className="text-xs">{t("is_default")}</span>
									</div>
								</div>
								<Button
									size="sm"
									className="w-full"
									disabled={!addPolicyId}
									onClick={handleAssign}
								>
									{t("common:confirm")}
								</Button>
							</div>
						)}
					</div>
				)}
			</DialogContent>
		</Dialog>
	);
}

export default function AdminUsersPage() {
	const { t } = useTranslation("admin");
	const [users, setUsers] = useState<UserInfo[]>([]);
	const [loading, setLoading] = useState(true);
	const [policyDialogUserId, setPolicyDialogUserId] = useState<number | null>(
		null,
	);

	const load = useCallback(async () => {
		try {
			setLoading(true);
			const data = await adminUserService.list();
			setUsers(data);
		} catch (e) {
			handleApiError(e);
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		load();
	}, [load]);

	const updateRole = async (id: number, role: UserRole) => {
		try {
			const updated = await adminUserService.update(id, { role });
			setUsers((prev) => prev.map((u) => (u.id === id ? updated : u)));
			toast.success(t("role_updated"));
		} catch (e) {
			handleApiError(e);
		}
	};

	const updateStatus = async (id: number, status: UserStatus) => {
		try {
			const updated = await adminUserService.update(id, { status });
			setUsers((prev) => prev.map((u) => (u.id === id ? updated : u)));
			toast.success(t("status_updated"));
		} catch (e) {
			handleApiError(e);
		}
	};

	const updateQuota = async (id: number, storage_quota: number) => {
		try {
			const updated = await adminUserService.update(id, {
				storage_quota,
			});
			setUsers((prev) => prev.map((u) => (u.id === id ? updated : u)));
			toast.success(t("quota_updated"));
		} catch (e) {
			handleApiError(e);
		}
	};

	return (
		<AdminLayout>
			<AdminPageShell>
				<AdminPageHeader title={t("users")} description={t("no_users")} />
				{loading ? (
					<SkeletonTable columns={8} rows={6} />
				) : users.length === 0 ? (
					<EmptyState title={t("no_users")} />
				) : (
					<AdminSurface>
						<ScrollArea className="min-h-0 flex-1">
							<Table>
								<TableHeader>
									<TableRow>
										<TableHead className="w-16">{t("id")}</TableHead>
										<TableHead>{t("username")}</TableHead>
										<TableHead>{t("email")}</TableHead>
										<TableHead className="w-32">{t("role")}</TableHead>
										<TableHead className="w-32">{t("common:status")}</TableHead>
										<TableHead className="w-40">{t("storage")}</TableHead>
										<TableHead className="w-24">{t("policies")}</TableHead>
										<TableHead>{t("common:created_at")}</TableHead>
									</TableRow>
								</TableHeader>
								<TableBody>
									{users.map((user) => (
										<TableRow key={user.id}>
											<TableCell className="font-mono text-xs">
												{user.id}
											</TableCell>
											<TableCell className="font-medium">
												{user.username}
											</TableCell>
											<TableCell className="text-muted-foreground">
												{user.email}
											</TableCell>
											<TableCell>
												<Select
													value={user.role}
													onValueChange={(v) =>
														updateRole(user.id, v as UserRole)
													}
												>
													<SelectTrigger className="h-8 w-24">
														<SelectValue />
													</SelectTrigger>
													<SelectContent>
														<SelectItem value="admin">Admin</SelectItem>
														<SelectItem value="user">User</SelectItem>
													</SelectContent>
												</Select>
											</TableCell>
											<TableCell>
												<Select
													value={user.status}
													onValueChange={(v) =>
														updateStatus(user.id, v as UserStatus)
													}
												>
													<SelectTrigger className="h-8 w-28">
														<SelectValue />
													</SelectTrigger>
													<SelectContent>
														<SelectItem value="active">
															<Badge
																variant="outline"
																className="border-green-600 text-green-600 dark:border-green-400 dark:text-green-400"
															>
																{t("common:active")}
															</Badge>
														</SelectItem>
														<SelectItem value="disabled">
															<Badge
																variant="outline"
																className="border-red-600 text-red-600 dark:border-red-400 dark:text-red-400"
															>
																{t("common:disabled_status")}
															</Badge>
														</SelectItem>
													</SelectContent>
												</Select>
											</TableCell>
											<TableCell>
												<QuotaCell user={user} onUpdate={updateQuota} />
											</TableCell>
											<TableCell>
												<Button
													variant="ghost"
													size="sm"
													className="h-8"
													onClick={() => setPolicyDialogUserId(user.id)}
												>
													<Icon name="HardDrive" className="mr-1 h-3.5 w-3.5" />
													{t("common:manage")}
												</Button>
											</TableCell>
											<TableCell className="text-muted-foreground text-xs">
												{new Date(user.created_at).toLocaleDateString()}
											</TableCell>
										</TableRow>
									))}
								</TableBody>
							</Table>
						</ScrollArea>
					</AdminSurface>
				)}
			</AdminPageShell>
			{policyDialogUserId !== null && (
				<UserPolicyDialog
					userId={policyDialogUserId}
					open={true}
					onOpenChange={(open) => {
						if (!open) setPolicyDialogUserId(null);
					}}
				/>
			)}
		</AdminLayout>
	);
}
