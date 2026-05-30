import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { AdminTableList } from "@/components/common/AdminTableList";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { UserIdentity } from "@/components/common/UserIdentity";
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
import { handleApiError } from "@/hooks/useApiError";
import { useApiList } from "@/hooks/useApiList";
import { useConfirmDialog } from "@/hooks/useConfirmDialog";
import { usePendingId } from "@/hooks/usePendingId";
import { useRetainedDialogValue } from "@/hooks/useRetainedDialogValue";
import { writeTextToClipboard } from "@/lib/clipboard";
import { FOLDER_LIMIT } from "@/lib/constants";
import { formatDateShort } from "@/lib/format";
import { absoluteAppUrl } from "@/lib/publicSiteUrl";
import { webdavEndpointPath } from "@/lib/webdav";
import { createFileService } from "@/services/fileService";
import { webdavAccountService } from "@/services/webdavAccountService";
import type { FolderListItem } from "@/types/api";

interface TeamManageWebdavSectionProps {
	canManageTeam: boolean;
	currentUserId: number | null;
	teamId: number;
	webdavPrefix: string;
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
				type="button"
				variant="outline"
				size={copyLabel ? "default" : "icon-sm"}
				className="sm:shrink-0"
				onClick={onCopy}
			>
				<Icon name="Copy" className="size-3.5" />
				{copyLabel ? copyLabel : null}
			</Button>
		</div>
	);
}

export function TeamManageWebdavSection({
	canManageTeam,
	currentUserId,
	teamId,
	webdavPrefix,
}: TeamManageWebdavSectionProps) {
	const { t } = useTranslation([
		"core",
		"admin",
		"settings",
		"webdav",
		"errors",
	]);
	const teamFileService = useMemo(
		() => createFileService({ kind: "team", teamId }),
		[teamId],
	);
	const {
		items: accounts,
		loading,
		reload,
	} = useApiList(
		() => webdavAccountService.listForTeam(teamId, { limit: 200, offset: 0 }),
		[teamId],
	);
	const [folders, setFolders] = useState<FolderListItem[]>([]);
	const [createDialogOpen, setCreateDialogOpen] = useState(false);
	const [credentialsDialogOpen, setCredentialsDialogOpen] = useState(false);
	const [creating, setCreating] = useState(false);
	const [newUsername, setNewUsername] = useState("");
	const [newPassword, setNewPassword] = useState("");
	const [selectedFolderId, setSelectedFolderId] = useState<number | undefined>(
		undefined,
	);
	const [showPassword, setShowPassword] = useState<{
		username: string;
		password: string;
	} | null>(null);
	const [testing, setTesting] = useState(false);
	const [testResult, setTestResult] = useState<boolean | null>(null);
	const {
		pendingId: deletingAccountId,
		runWithPending: runWithDeletingAccount,
	} = usePendingId<number>();
	const {
		pendingId: togglingAccountId,
		runWithPending: runWithTogglingAccount,
	} = usePendingId<number>();
	const {
		retainedValue: recentCredentials,
		handleOpenChangeComplete: handleCredentialsDialogOpenChangeComplete,
	} = useRetainedDialogValue(showPassword, credentialsDialogOpen);

	const fetchFolders = useCallback(async () => {
		try {
			const data = await teamFileService.listRoot({
				file_limit: 0,
				folder_limit: FOLDER_LIMIT,
			});
			setFolders(data.folders);
		} catch (err) {
			handleApiError(err);
		}
	}, [teamFileService]);

	useEffect(() => {
		void fetchFolders();
	}, [fetchFolders]);

	const endpointUrl = absoluteAppUrl(webdavEndpointPath(webdavPrefix));
	const rootFolderOptions = [
		{
			label: t("webdav:all_files_full_access"),
			value: "__all__",
		},
		...folders.map((folder) => ({
			label: `/${folder.name}`,
			value: String(folder.id),
		})),
	];
	const sortedAccounts = useMemo(
		() =>
			accounts.toSorted(
				(a, b) =>
					new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
			),
		[accounts],
	);

	const copyToClipboard = useCallback(
		async (value: string) => {
			try {
				await writeTextToClipboard(value);
				toast.success(t("core:copied_to_clipboard"));
			} catch {
				toast.error(t("errors:unexpected_error"));
			}
		},
		[t],
	);

	const handleCreate = async () => {
		if (!newUsername.trim()) {
			toast.error(t("webdav:username_required"));
			return;
		}

		setCreating(true);
		try {
			const result = await webdavAccountService.createForTeam(teamId, {
				username: newUsername.trim(),
				password: newPassword.trim() || undefined,
				root_folder_id: selectedFolderId ?? null,
			});
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
			void reload();
		} catch (err) {
			handleApiError(err);
		} finally {
			setCreating(false);
		}
	};

	const handleDelete = async (id: number) => {
		await runWithDeletingAccount(id, async () => {
			try {
				await webdavAccountService.deleteForTeam(teamId, id);
				toast.success(t("admin:webdav_account_deleted"));
				void reload();
			} catch (err) {
				handleApiError(err);
			}
		});
	};

	const { requestConfirm, dialogProps } = useConfirmDialog(handleDelete);

	const handleToggle = async (id: number) => {
		await runWithTogglingAccount(id, async () => {
			try {
				await webdavAccountService.toggleForTeam(teamId, id);
				void reload();
			} catch (err) {
				handleApiError(err);
			}
		});
	};

	const handleTest = async () => {
		if (!recentCredentials) return;
		setTesting(true);
		setTestResult(null);
		try {
			await webdavAccountService.test({
				username: recentCredentials.username,
				password: recentCredentials.password,
			});
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
		<section className="rounded-2xl border bg-background/60 p-6">
			<div className="mb-5 flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
				<div>
					<h4 className="text-base font-semibold text-foreground">
						{t("settings:settings_team_webdav_title")}
					</h4>
					<p className="mt-1 text-sm text-muted-foreground">
						{canManageTeam
							? t("settings:settings_team_webdav_desc_manager")
							: t("settings:settings_team_webdav_desc_member")}
					</p>
				</div>
				<Button type="button" onClick={() => setCreateDialogOpen(true)}>
					<Icon name="Plus" className="size-4" />
					{t("webdav:create_webdav_account")}
				</Button>
			</div>

			<div className="mb-4 rounded-xl border bg-muted/20 p-4">
				<div className="mb-1 flex items-center gap-2">
					<Icon name="Globe" className="size-4 text-muted-foreground" />
					<p className="text-sm font-medium">{t("webdav:webdav_endpoint")}</p>
				</div>
				<p className="mb-3 text-xs text-muted-foreground">
					{t("settings:settings_team_webdav_endpoint_hint")}
				</p>
				<CopyField
					value={endpointUrl}
					onCopy={() => void copyToClipboard(endpointUrl)}
					copyLabel={t("webdav:webdav_copy_endpoint")}
				/>
			</div>

			<AdminTableList
				loading={loading}
				items={sortedAccounts}
				columns={canManageTeam ? 6 : 5}
				rows={5}
				emptyIcon={<Icon name="Globe" className="size-10" />}
				emptyTitle={t("webdav:no_webdav_accounts")}
				emptyDescription={t("settings:settings_team_webdav_empty_desc")}
				headerRow={
					<TableHeader>
						<TableRow>
							<TableHead>{t("core:username")}</TableHead>
							{canManageTeam ? (
								<TableHead>
									{t("settings:settings_team_webdav_owner")}
								</TableHead>
							) : null}
							<TableHead>{t("webdav:access_scope")}</TableHead>
							<TableHead>{t("core:status")}</TableHead>
							<TableHead>{t("core:created_at")}</TableHead>
							<TableHead className="w-[96px] text-right">
								{t("core:actions")}
							</TableHead>
						</TableRow>
					</TableHeader>
				}
				renderRow={(account) => {
					const isDeleting = deletingAccountId === account.id;
					const isToggling = togglingAccountId === account.id;
					const canMutateAccount =
						canManageTeam || account.user_id === currentUserId;
					const deleteLabel = isDeleting
						? t("admin:webdav_account_deleting")
						: t("core:delete");
					const toggleLabel = isToggling
						? t("admin:webdav_account_updating")
						: account.is_active
							? t("core:disabled_status")
							: t("core:active");

					return (
						<TableRow key={account.id}>
							<TableCell>
								<div className="min-w-[140px]">
									<span className="truncate font-mono text-sm font-medium text-foreground">
										{account.username}
									</span>
								</div>
							</TableCell>
							{canManageTeam ? (
								<TableCell>
									<UserIdentity
										user={account.user}
										fallbackLabel={`#${account.user_id}`}
									/>
								</TableCell>
							) : null}
							<TableCell>
								<div className="flex min-w-[180px] items-center gap-2 text-sm text-foreground">
									<Icon
										name={account.root_folder_path ? "FolderOpen" : "Globe"}
										className="size-3.5 shrink-0 text-muted-foreground"
									/>
									<span className="truncate">
										{account.root_folder_path ?? t("core:all_files")}
									</span>
								</div>
							</TableCell>
							<TableCell>
								<Badge
									variant={account.is_active ? "secondary" : "outline"}
									className={
										account.is_active
											? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
											: undefined
									}
								>
									{account.is_active
										? t("core:active")
										: t("core:disabled_status")}
								</Badge>
							</TableCell>
							<TableCell className="text-sm text-muted-foreground">
								{formatDateShort(account.created_at)}
							</TableCell>
							<TableCell>
								<div className="flex justify-end gap-2">
									<Button
										type="button"
										variant="outline"
										size="icon-sm"
										onClick={() => void handleToggle(account.id)}
										title={toggleLabel}
										aria-label={toggleLabel}
										disabled={!canMutateAccount || isToggling || isDeleting}
									>
										<Icon
											name={isToggling ? "Spinner" : "Power"}
											className={`size-3.5 ${isToggling ? "animate-spin" : ""}`}
										/>
									</Button>
									<Button
										type="button"
										variant="destructive"
										size="icon-sm"
										onClick={() => requestConfirm(account.id)}
										title={deleteLabel}
										aria-label={deleteLabel}
										disabled={!canMutateAccount || isDeleting || isToggling}
									>
										<Icon
											name={isDeleting ? "Spinner" : "Trash"}
											className={`size-3.5 ${isDeleting ? "animate-spin" : ""}`}
										/>
									</Button>
								</div>
							</TableCell>
						</TableRow>
					);
				}}
			/>

			<Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
				<DialogContent className="max-w-md">
					<DialogHeader>
						<DialogTitle>{t("webdav:create_webdav_account")}</DialogTitle>
						<DialogDescription>
							{t("settings:settings_team_webdav_create_desc")}
						</DialogDescription>
					</DialogHeader>
					<div className="space-y-4 py-2">
						<div className="space-y-1.5">
							<Label htmlFor="team-webdav-username">{t("core:username")}</Label>
							<Input
								id="team-webdav-username"
								value={newUsername}
								onChange={(event) => setNewUsername(event.target.value)}
								placeholder={t("webdav:webdav_username_placeholder")}
							/>
						</div>
						<div className="space-y-1.5">
							<Label htmlFor="team-webdav-password">{t("core:password")}</Label>
							<Input
								id="team-webdav-password"
								type="password"
								value={newPassword}
								onChange={(event) => setNewPassword(event.target.value)}
								placeholder={t("webdav:auto_generate_password")}
							/>
							<p className="text-xs text-muted-foreground">
								{t("webdav:auto_generate_password")}
							</p>
						</div>
						<div className="space-y-1.5">
							<Label htmlFor="team-webdav-root-folder">
								{t("webdav:access_scope")}
							</Label>
							<Select
								items={rootFolderOptions}
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
								<SelectTrigger id="team-webdav-root-folder">
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									{rootFolderOptions.map((option) => (
										<SelectItem key={option.value} value={option.value}>
											{option.label}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
							{folders.length === 0 ? (
								<p className="text-xs text-muted-foreground">
									{t("webdav:webdav_no_root_folders")}
								</p>
							) : null}
						</div>
					</div>
					<DialogFooter>
						<Button
							type="button"
							onClick={() => void handleCreate()}
							disabled={creating || !newUsername.trim()}
						>
							<Icon name={creating ? "Spinner" : "Plus"} className="size-4" />
							{creating ? t("core:loading") : t("core:create")}
						</Button>
					</DialogFooter>
				</DialogContent>
			</Dialog>

			{recentCredentials ? (
				<Dialog
					open={credentialsDialogOpen}
					onOpenChange={(open) => {
						setCredentialsDialogOpen(open);
						if (!open) {
							setShowPassword(null);
						}
					}}
					onOpenChangeComplete={(open) => {
						handleCredentialsDialogOpenChangeComplete(open);
						if (!open) {
							setTestResult(null);
						}
					}}
				>
					<DialogContent className="max-w-md">
						<DialogHeader>
							<DialogTitle>{t("webdav:webdav_recent_credentials")}</DialogTitle>
							<DialogDescription>
								{t("webdav:webdav_recent_credentials_desc")}
							</DialogDescription>
						</DialogHeader>
						<div className="space-y-4 py-2">
							<div className="space-y-1.5">
								<Label>{t("core:username")}</Label>
								<CopyField
									value={recentCredentials.username}
									onCopy={() =>
										void copyToClipboard(recentCredentials.username)
									}
								/>
							</div>
							<div className="space-y-1.5">
								<Label>{t("core:password")}</Label>
								<CopyField
									value={recentCredentials.password}
									onCopy={() =>
										void copyToClipboard(recentCredentials.password)
									}
								/>
							</div>
							{testResult !== null ? (
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
							) : null}
						</div>
						<DialogFooter>
							<Button
								type="button"
								variant="outline"
								onClick={() => void handleTest()}
								disabled={testing}
							>
								{testing ? (
									<Icon name="Spinner" className="size-4 animate-spin" />
								) : (
									<Icon name="WifiHigh" className="size-4" />
								)}
								{t("admin:test_connection")}
							</Button>
						</DialogFooter>
					</DialogContent>
				</Dialog>
			) : null}

			<ConfirmDialog
				{...dialogProps}
				title={t("core:are_you_sure")}
				description={t("core:cannot_undo")}
				confirmLabel={t("core:delete")}
				variant="destructive"
			/>
		</section>
	);
}
