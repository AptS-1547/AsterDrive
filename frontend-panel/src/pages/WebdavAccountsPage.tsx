import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AppLayout } from "@/components/layout/AppLayout";
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
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { fileService } from "@/services/fileService";
import { webdavAccountService } from "@/services/webdavAccountService";
import type { FolderInfo, WebdavAccountInfo } from "@/types/api";

export default function WebdavAccountsPage() {
	const { t } = useTranslation();
	const [accounts, setAccounts] = useState<WebdavAccountInfo[]>([]);
	const [loading, setLoading] = useState(true);
	const [creating, setCreating] = useState(false);
	const [newUsername, setNewUsername] = useState("");
	const [newPassword, setNewPassword] = useState("");
	const [selectedFolderId, setSelectedFolderId] = useState<number | undefined>(
		undefined,
	);
	const [folders, setFolders] = useState<FolderInfo[]>([]);
	const [showPassword, setShowPassword] = useState<{
		username: string;
		password: string;
	} | null>(null);
	const [testing, setTesting] = useState(false);
	const [testResult, setTestResult] = useState<boolean | null>(null);
	const [deleteId, setDeleteId] = useState<number | null>(null);

	const fetchAccounts = useCallback(async () => {
		try {
			const data = await webdavAccountService.list();
			setAccounts(data);
		} catch (err) {
			handleApiError(err);
		} finally {
			setLoading(false);
		}
	}, []);

	const fetchFolders = useCallback(async () => {
		try {
			const data = await fileService.listRoot();
			setFolders(data.folders);
		} catch (err) {
			handleApiError(err);
		}
	}, []);

	useEffect(() => {
		fetchAccounts();
		fetchFolders();
	}, [fetchAccounts, fetchFolders]);

	const handleCreate = async () => {
		if (!newUsername.trim()) {
			toast.error(t("username_required"));
			return;
		}
		setCreating(true);
		try {
			const result = await webdavAccountService.create(
				newUsername.trim(),
				newPassword.trim() || undefined,
				selectedFolderId,
			);
			setShowPassword({
				username: result.username,
				password: result.password,
			});
			setTestResult(null);
			setNewUsername("");
			setNewPassword("");
			setSelectedFolderId(undefined);
			toast.success(t("admin:webdav_account_created"));
			fetchAccounts();
		} catch (err) {
			handleApiError(err);
		} finally {
			setCreating(false);
		}
	};

	const handleDelete = async (id: number) => {
		try {
			await webdavAccountService.delete(id);
			toast.success(t("admin:webdav_account_deleted"));
			fetchAccounts();
		} catch (err) {
			handleApiError(err);
		}
	};

	const handleToggle = async (id: number) => {
		try {
			await webdavAccountService.toggle(id);
			fetchAccounts();
		} catch (err) {
			handleApiError(err);
		}
	};

	const handleTest = async () => {
		if (!showPassword) return;
		setTesting(true);
		setTestResult(null);
		try {
			await webdavAccountService.test(
				showPassword.username,
				showPassword.password,
			);
			setTestResult(true);
			toast.success(t("admin:connection_success"));
		} catch {
			setTestResult(false);
			toast.error(t("admin:connection_test_failed"));
		} finally {
			setTesting(false);
		}
	};

	const copyToClipboard = (text: string) => {
		navigator.clipboard.writeText(text);
		toast.success(t("copied_to_clipboard"));
	};

	return (
		<AppLayout>
			<div className="flex-1 overflow-auto p-6 space-y-6">
				{/* Create form */}
				<div className="border rounded-lg p-4 space-y-4 max-w-lg">
					<h3 className="font-medium">{t("create_webdav_account")}</h3>
					<div className="space-y-2">
						<Label htmlFor="username">{t("admin:username")}</Label>
						<Input
							id="username"
							value={newUsername}
							onChange={(e) => setNewUsername(e.target.value)}
							placeholder="e.g. my-webdav"
						/>
					</div>
					<div className="space-y-2">
						<Label htmlFor="password">
							{t("auth:password")}{" "}
							<span className="text-muted-foreground font-normal">
								({t("auto_generate_password")})
							</span>
						</Label>
						<Input
							id="password"
							type="password"
							value={newPassword}
							onChange={(e) => setNewPassword(e.target.value)}
						/>
					</div>
					<div className="space-y-2">
						<Label htmlFor="rootFolder">
							{t("access_scope")}{" "}
							<span className="text-muted-foreground font-normal">
								({t("access_scope_desc")})
							</span>
						</Label>
						<Select
							value={
								selectedFolderId != null ? String(selectedFolderId) : "__all__"
							}
							onValueChange={(v) =>
								setSelectedFolderId(v === "__all__" ? undefined : Number(v))
							}
						>
							<SelectTrigger className="w-full">
								<SelectValue />
							</SelectTrigger>
							<SelectContent>
								<SelectItem value="__all__">
									{t("all_files_full_access")}
								</SelectItem>
								{folders.map((f) => (
									<SelectItem key={f.id} value={String(f.id)}>
										/{f.name}
									</SelectItem>
								))}
							</SelectContent>
						</Select>
					</div>
					<Button onClick={handleCreate} disabled={creating}>
						<Icon name="Plus" className="h-4 w-4 mr-1" />
						{t("create")}
					</Button>
				</div>

				{/* One-time password display */}
				{showPassword && (
					<div className="border border-yellow-500 bg-yellow-50 dark:bg-yellow-950 rounded-lg p-4 max-w-lg">
						<p className="text-sm font-medium mb-2">
							{t("save_password_warning")}
						</p>
						<div className="flex items-center gap-2">
							<code className="flex-1 bg-background px-3 py-2 rounded border font-mono text-sm">
								{showPassword.password}
							</code>
							<Button
								variant="outline"
								size="icon"
								onClick={() => copyToClipboard(showPassword.password)}
							>
								<Icon name="Copy" className="h-4 w-4" />
							</Button>
						</div>
						<div className="flex items-center gap-2 mt-2">
							<Button
								variant="outline"
								size="sm"
								disabled={testing}
								onClick={handleTest}
							>
								{testing ? (
									<Icon name="Spinner" className="h-4 w-4 mr-1 animate-spin" />
								) : testResult === true ? (
									<Icon
										name="Check"
										className="h-4 w-4 mr-1 text-green-600 dark:text-green-400"
									/>
								) : (
									<Icon name="WifiHigh" className="h-4 w-4 mr-1" />
								)}
								{t("admin:test_connection")}
							</Button>
							<Button
								variant="ghost"
								size="sm"
								onClick={() => setShowPassword(null)}
							>
								{t("dismiss")}
							</Button>
						</div>
					</div>
				)}

				{/* Accounts list */}
				{loading ? (
					<SkeletonTable columns={5} rows={6} />
				) : accounts.length === 0 ? (
					<EmptyState
						title={t("no_webdav_accounts")}
						description={t("no_webdav_accounts_desc")}
					/>
				) : (
					<Table>
						<TableHeader>
							<TableRow>
								<TableHead>{t("admin:username")}</TableHead>
								<TableHead>{t("access_scope")}</TableHead>
								<TableHead>{t("status")}</TableHead>
								<TableHead>{t("created_at")}</TableHead>
								<TableHead className="w-[120px]">{t("actions")}</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{accounts.map((acc) => (
								<TableRow key={acc.id}>
									<TableCell className="font-mono">{acc.username}</TableCell>
									<TableCell>
										{acc.root_folder_path ? (
											<span className="flex items-center gap-1 text-sm">
												<Icon
													name="FolderOpen"
													className="h-3.5 w-3.5 text-blue-500"
												/>
												{acc.root_folder_path}
											</span>
										) : (
											<span className="text-muted-foreground text-sm">
												{t("all_files")}
											</span>
										)}
									</TableCell>
									<TableCell>
										<span
											className={
												acc.is_active
													? "text-green-600 dark:text-green-400"
													: "text-red-500 dark:text-red-400"
											}
										>
											{acc.is_active ? t("active") : t("disabled_status")}
										</span>
									</TableCell>
									<TableCell className="text-muted-foreground">
										{new Date(acc.created_at).toLocaleDateString()}
									</TableCell>
									<TableCell>
										<div className="flex gap-1">
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8"
												onClick={() => handleToggle(acc.id)}
												title={
													acc.is_active ? t("disabled_status") : t("active")
												}
											>
												<Icon name="Power" className="h-4 w-4" />
											</Button>
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8 text-destructive"
												onClick={() => setDeleteId(acc.id)}
											>
												<Icon name="Trash" className="h-4 w-4" />
											</Button>
										</div>
									</TableCell>
								</TableRow>
							))}
						</TableBody>
					</Table>
				)}

				{/* Connection info */}
				<div className="border rounded-lg p-4 max-w-lg text-sm text-muted-foreground space-y-1">
					<p className="font-medium text-foreground">{t("connection_info")}</p>
					<p>
						URL:{" "}
						<code className="bg-muted px-1 rounded">
							{window.location.origin}/webdav/
						</code>
					</p>
					<p>Use your WebDAV username and password to connect.</p>
				</div>
			</div>

			<ConfirmDialog
				open={deleteId !== null}
				onOpenChange={(open) => {
					if (!open) setDeleteId(null);
				}}
				title={t("are_you_sure")}
				description={t("cannot_undo")}
				confirmLabel={t("delete")}
				onConfirm={() => {
					const id = deleteId;
					setDeleteId(null);
					if (id !== null) void handleDelete(id);
				}}
				variant="destructive"
			/>
		</AppLayout>
	);
}
