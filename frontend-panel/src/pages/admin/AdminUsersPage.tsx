import { HardDrive, Plus, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
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
import type { UserStoragePolicy } from "@/services/adminService";
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
} from "@/types/api";

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`;
}

function QuotaCell({
	user,
	onUpdate,
}: {
	user: UserInfo;
	onUpdate: (id: number, quota: number) => void;
}) {
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
				{quota > 0 ? ` / ${formatBytes(quota)}` : " / Unlimited"}
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
			toast.success("Policy assigned");
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
			toast.success("Assignment removed");
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
					<DialogTitle>Storage Policy Assignments</DialogTitle>
				</DialogHeader>

				{loading ? (
					<p className="text-sm text-muted-foreground">Loading...</p>
				) : (
					<div className="space-y-4">
						{/* Existing assignments */}
						{assignments.length === 0 ? (
							<p className="text-sm text-muted-foreground">
								No policies assigned. Using system default.
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
												Quota:{" "}
												{a.quota_bytes > 0
													? formatBytes(a.quota_bytes)
													: "Unlimited"}
											</div>
										</div>
										<div className="flex items-center gap-2">
											{a.is_default && (
												<Badge className="bg-blue-100 text-blue-700 border-blue-300">
													Default
												</Badge>
											)}
											<Button
												variant="ghost"
												size="sm"
												onClick={() => handleToggleDefault(a)}
											>
												{a.is_default ? "Unset default" : "Set default"}
											</Button>
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8 text-destructive"
												onClick={() => handleRemove(a.id)}
											>
												<Trash2 className="h-3.5 w-3.5" />
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
									<Plus className="h-3.5 w-3.5 inline mr-1" />
									Assign Policy
								</Label>
								<Select
									value={addPolicyId != null ? String(addPolicyId) : ""}
									onValueChange={(v) => setAddPolicyId(v ? Number(v) : null)}
								>
									<SelectTrigger>
										<SelectValue placeholder="Select policy..." />
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
											placeholder="Quota (MB, 0=unlimited)"
											value={addQuota}
											onChange={(e) => setAddQuota(e.target.value)}
										/>
									</div>
									<div className="flex items-center gap-2">
										<Switch
											checked={addDefault}
											onCheckedChange={setAddDefault}
										/>
										<span className="text-xs">Default</span>
									</div>
								</div>
								<Button
									size="sm"
									className="w-full"
									disabled={!addPolicyId}
									onClick={handleAssign}
								>
									Assign
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
			toast.success("Role updated");
		} catch (e) {
			handleApiError(e);
		}
	};

	const updateStatus = async (id: number, status: UserStatus) => {
		try {
			const updated = await adminUserService.update(id, { status });
			setUsers((prev) => prev.map((u) => (u.id === id ? updated : u)));
			toast.success("Status updated");
		} catch (e) {
			handleApiError(e);
		}
	};

	const updateQuota = async (id: number, storage_quota: number) => {
		try {
			const updated = await adminUserService.update(id, { storage_quota });
			setUsers((prev) => prev.map((u) => (u.id === id ? updated : u)));
			toast.success("Quota updated");
		} catch (e) {
			handleApiError(e);
		}
	};

	return (
		<AdminLayout>
			<div className="p-6 space-y-4">
				<h2 className="text-lg font-semibold">Users</h2>
				<ScrollArea className="flex-1">
					<Table>
						<TableHeader>
							<TableRow>
								<TableHead className="w-16">ID</TableHead>
								<TableHead>Username</TableHead>
								<TableHead>Email</TableHead>
								<TableHead className="w-32">Role</TableHead>
								<TableHead className="w-32">Status</TableHead>
								<TableHead className="w-40">Storage</TableHead>
								<TableHead className="w-24">Policies</TableHead>
								<TableHead>Created</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{loading ? (
								<TableRow>
									<TableCell
										colSpan={8}
										className="text-center text-muted-foreground"
									>
										Loading...
									</TableCell>
								</TableRow>
							) : users.length === 0 ? (
								<TableRow>
									<TableCell
										colSpan={8}
										className="text-center text-muted-foreground"
									>
										No users
									</TableCell>
								</TableRow>
							) : (
								users.map((user) => (
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
															className="text-green-600 border-green-600"
														>
															Active
														</Badge>
													</SelectItem>
													<SelectItem value="disabled">
														<Badge
															variant="outline"
															className="text-red-600 border-red-600"
														>
															Disabled
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
												<HardDrive className="h-3.5 w-3.5 mr-1" />
												Manage
											</Button>
										</TableCell>
										<TableCell className="text-muted-foreground text-xs">
											{new Date(user.created_at).toLocaleDateString()}
										</TableCell>
									</TableRow>
								))
							)}
						</TableBody>
					</Table>
				</ScrollArea>
			</div>
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
