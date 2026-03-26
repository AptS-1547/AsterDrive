import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AdminSurface } from "@/components/layout/AdminSurface";
import { AppLayout } from "@/components/layout/AppLayout";
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
import { ScrollArea } from "@/components/ui/scroll-area";
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
import { PAGE_SECTION_PADDING_CLASS } from "@/lib/constants";
import { fileService } from "@/services/fileService";
import { webdavAccountService } from "@/services/webdavAccountService";
import type { FolderInfo, WebdavAccountInfo } from "@/types/api";

function formatDateOnly(value: string) {
	return new Date(value).toLocaleDateString();
}

function CopyField({
	value,
	onCopy,
	copyLabel,
}: {
	value: string;
	onCopy: () => void;
	copyLabel?: string;
}) {
	return (
		<div className="flex flex-col gap-2 sm:flex-row">
			<Input readOnly value={value} className="font-mono" />
			<Button
				variant="outline"
				size={copyLabel ? "default" : "icon-sm"}
				className="sm:shrink-0"
				onClick={onCopy}
			>
				<Icon name="Copy" className="h-3.5 w-3.5" />
				{copyLabel ? copyLabel : null}
			</Button>
		</div>
	);
}

export default function WebdavAccountsPage() {
	const { t } = useTranslation(["common", "admin", "auth"]);
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
	const [createDialogOpen, setCreateDialogOpen] = useState(false);
	const [credentialsDialogOpen, setCredentialsDialogOpen] = useState(false);

	const fetchAccounts = useCallback(async (showLoader = true) => {
		if (showLoader) setLoading(true);
		try {
			const data = await webdavAccountService.list({ limit: 200, offset: 0 });
			setAccounts(data.items);
		} catch (err) {
			handleApiError(err);
		} finally {
			if (showLoader) setLoading(false);
		}
	}, []);

	const fetchFolders = useCallback(async () => {
		try {
			const data = await fileService.listRoot({
				file_limit: 0,
				folder_limit: 1000,
			});
			setFolders(data.folders);
		} catch (err) {
			handleApiError(err);
		}
	}, []);

	useEffect(() => {
		void fetchAccounts();
		void fetchFolders();
	}, [fetchAccounts, fetchFolders]);

	const endpointUrl =
		typeof window === "undefined"
			? "/webdav/"
			: `${window.location.origin}/webdav/`;
	const sortedAccounts = useMemo(
		() =>
			[...accounts].sort(
				(a, b) =>
					new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
			),
		[accounts],
	);

	const copyToClipboard = useCallback(
		async (value: string) => {
			try {
				await navigator.clipboard.writeText(value);
				toast.success(t("common:copied_to_clipboard"));
			} catch {
				toast.error(t("common:unexpected_error"));
			}
		},
		[t],
	);

	const handleCreate = async () => {
		if (!newUsername.trim()) {
			toast.error(t("common:username_required"));
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
			setCreateDialogOpen(false);
			setCredentialsDialogOpen(true);
			toast.success(t("admin:webdav_account_created"));
			await fetchAccounts(false);
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
			await fetchAccounts(false);
		} catch (err) {
			handleApiError(err);
		}
	};

	const handleToggle = async (id: number) => {
		try {
			await webdavAccountService.toggle(id);
			await fetchAccounts(false);
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

	return (
		<AppLayout>
			{/* Create Account Dialog */}
			<Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
				<DialogContent className="max-w-md">
					<DialogHeader>
						<DialogTitle>{t("common:create_webdav_account")}</DialogTitle>
						<DialogDescription>
							{t("common:webdav_test_hint")}
						</DialogDescription>
					</DialogHeader>
					<div className="space-y-4 py-2">
						{/* username field */}
						<div className="space-y-1.5">
							<Label htmlFor="username">{t("admin:username")}</Label>
							<Input
								id="username"
								value={newUsername}
								onChange={(e) => setNewUsername(e.target.value)}
								placeholder={t("common:webdav_username_placeholder")}
							/>
						</div>
						{/* password field */}
						<div className="space-y-1.5">
							<Label htmlFor="password">{t("auth:password")}</Label>
							<Input
								id="password"
								type="password"
								value={newPassword}
								onChange={(e) => setNewPassword(e.target.value)}
								placeholder={t("common:auto_generate_password")}
							/>
							<p className="text-xs text-muted-foreground">
								{t("common:auto_generate_password")}
							</p>
						</div>
						{/* access scope select */}
						<div className="space-y-1.5">
							<Label htmlFor="rootFolder">{t("common:access_scope")}</Label>
							<Select
								value={
									selectedFolderId != null
										? String(selectedFolderId)
										: "__all__"
								}
								onValueChange={(value) =>
									setSelectedFolderId(
										value === "__all__" ? undefined : Number(value),
									)
								}
							>
								<SelectTrigger id="rootFolder" className="w-full">
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									<SelectItem value="__all__">
										{t("common:all_files_full_access")}
									</SelectItem>
									{folders.map((folder) => (
										<SelectItem key={folder.id} value={String(folder.id)}>
											/{folder.name}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
							{folders.length === 0 && (
								<p className="text-xs text-muted-foreground">
									{t("common:webdav_no_root_folders")}
								</p>
							)}
						</div>
					</div>
					<DialogFooter>
						<Button
							onClick={() => void handleCreate()}
							disabled={creating || !newUsername.trim()}
						>
							<Icon name="Plus" className="h-4 w-4" />
							{creating ? t("common:loading") : t("common:create")}
						</Button>
					</DialogFooter>
				</DialogContent>
			</Dialog>

			{/* Credentials Dialog - shows after successful creation */}
			{showPassword && (
				<Dialog
					open={credentialsDialogOpen}
					onOpenChange={(open) => {
						if (!open) {
							setCredentialsDialogOpen(false);
							setShowPassword(null);
							setTestResult(null);
						}
					}}
				>
					<DialogContent className="max-w-md">
						<DialogHeader>
							<DialogTitle>{t("common:webdav_recent_credentials")}</DialogTitle>
							<DialogDescription>
								{t("common:webdav_recent_credentials_desc")}
							</DialogDescription>
						</DialogHeader>
						<div className="space-y-4 py-2">
							<div className="space-y-1.5">
								<Label>{t("admin:username")}</Label>
								<CopyField
									value={showPassword.username}
									onCopy={() => void copyToClipboard(showPassword.username)}
								/>
							</div>
							<div className="space-y-1.5">
								<Label>{t("auth:password")}</Label>
								<CopyField
									value={showPassword.password}
									onCopy={() => void copyToClipboard(showPassword.password)}
								/>
							</div>
							{testResult !== null && (
								<Badge
									variant={testResult ? "secondary" : "destructive"}
									className={
										testResult
											? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
											: undefined
									}
								>
									{testResult
										? t("admin:connection_success")
										: t("admin:connection_test_failed")}
								</Badge>
							)}
						</div>
						<DialogFooter>
							<Button
								variant="outline"
								onClick={() => void handleTest()}
								disabled={testing}
							>
								{testing ? (
									<Icon name="Spinner" className="h-4 w-4 animate-spin" />
								) : (
									<Icon name="WifiHigh" className="h-4 w-4" />
								)}
								{t("admin:test_connection")}
							</Button>
						</DialogFooter>
					</DialogContent>
				</Dialog>
			)}

			<div className="min-h-0 flex-1 overflow-auto">
				<div
					className={`flex flex-col gap-4 py-4 md:py-6 ${PAGE_SECTION_PADDING_CLASS}`}
				>
					{/* Page Header */}
					<div className="flex items-start justify-between gap-4">
						<div>
							<h1 className="text-xl font-semibold">{t("common:webdav")}</h1>
							<p className="mt-1 text-sm text-muted-foreground">
								{t("common:webdav_page_desc")}
							</p>
						</div>
						<Button
							className="shrink-0"
							onClick={() => setCreateDialogOpen(true)}
						>
							<Icon name="Plus" className="h-4 w-4" />
							{t("common:create_webdav_account")}
						</Button>
					</div>

					{/* Endpoint Info Card */}
					<div className="rounded-xl border bg-muted/20 p-4">
						<div className="flex items-center gap-2 mb-1">
							<Icon name="Globe" className="h-4 w-4 text-muted-foreground" />
							<p className="text-sm font-medium">
								{t("common:webdav_endpoint")}
							</p>
						</div>
						<p className="mb-3 text-xs text-muted-foreground">
							{t("common:webdav_use_credentials_hint")}
						</p>
						<CopyField
							value={endpointUrl}
							onCopy={() => void copyToClipboard(endpointUrl)}
							copyLabel={t("common:webdav_copy_endpoint")}
						/>
					</div>

					{/* Accounts Table */}
					<AdminSurface>
						{loading ? (
							<SkeletonTable columns={4} rows={5} />
						) : sortedAccounts.length === 0 ? (
							<EmptyState
								icon={<Icon name="Globe" className="h-10 w-10" />}
								title={t("common:no_webdav_accounts")}
								description={t("common:no_webdav_accounts_desc")}
							/>
						) : (
							<ScrollArea className="min-h-0 flex-1">
								<Table>
									<TableHeader>
										<TableRow>
											<TableHead>{t("admin:username")}</TableHead>
											<TableHead>{t("common:access_scope")}</TableHead>
											<TableHead>{t("common:status")}</TableHead>
											<TableHead>{t("common:created_at")}</TableHead>
											<TableHead className="w-[96px] text-right">
												{t("common:actions")}
											</TableHead>
										</TableRow>
									</TableHeader>
									<TableBody>
										{sortedAccounts.map((account) => (
											<TableRow key={account.id}>
												<TableCell>
													<div className="min-w-[140px]">
														<span className="truncate font-mono text-sm font-medium text-foreground">
															{account.username}
														</span>
													</div>
												</TableCell>
												<TableCell>
													<div className="flex min-w-[180px] items-center gap-2 text-sm text-foreground">
														<Icon
															name={
																account.root_folder_path
																	? "FolderOpen"
																	: "Globe"
															}
															className="h-3.5 w-3.5 shrink-0 text-muted-foreground"
														/>
														<span className="truncate">
															{account.root_folder_path ??
																t("common:all_files")}
														</span>
													</div>
												</TableCell>
												<TableCell>
													<Badge
														variant={
															account.is_active ? "secondary" : "outline"
														}
														className={
															account.is_active
																? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
																: undefined
														}
													>
														{account.is_active
															? t("common:active")
															: t("common:disabled_status")}
													</Badge>
												</TableCell>
												<TableCell className="text-sm text-muted-foreground">
													{formatDateOnly(account.created_at)}
												</TableCell>
												<TableCell>
													<div className="flex justify-end gap-2">
														<Button
															variant="outline"
															size="icon-sm"
															onClick={() => void handleToggle(account.id)}
															title={
																account.is_active
																	? t("common:disabled_status")
																	: t("common:active")
															}
														>
															<Icon name="Power" className="h-3.5 w-3.5" />
														</Button>
														<Button
															variant="destructive"
															size="icon-sm"
															onClick={() => setDeleteId(account.id)}
															title={t("common:delete")}
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
					</AdminSurface>
				</div>
			</div>

			<ConfirmDialog
				open={deleteId !== null}
				onOpenChange={(open) => {
					if (!open) setDeleteId(null);
				}}
				title={t("common:are_you_sure")}
				description={t("common:cannot_undo")}
				confirmLabel={t("common:delete")}
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
