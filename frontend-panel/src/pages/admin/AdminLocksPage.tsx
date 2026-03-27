import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { AdminTableList } from "@/components/common/AdminTableList";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { StatusBadge } from "@/components/common/StatusBadge";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { AdminPageHeader } from "@/components/layout/AdminPageHeader";
import { AdminPageShell } from "@/components/layout/AdminPageShell";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import {
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { useApiList } from "@/hooks/useApiList";
import { useConfirmDialog } from "@/hooks/useConfirmDialog";
import {
	ADMIN_ICON_BUTTON_CLASS,
	ADMIN_TABLE_ACTIONS_WIDTH_CLASS,
} from "@/lib/constants";
import { formatDateShort } from "@/lib/format";
import type { WebdavLock } from "@/services/adminService";
import { adminLockService } from "@/services/adminService";

export default function AdminLocksPage() {
	const { t } = useTranslation("admin");
	const {
		items: locks,
		setItems: setLocks,
		loading,
		reload,
	} = useApiList(() => adminLockService.list({ limit: 100, offset: 0 }));

	const handleForceUnlock = async (id: number) => {
		try {
			await adminLockService.forceUnlock(id);
			setLocks((prev) => prev.filter((l) => l.id !== id));
			toast.success(t("lock_released"));
		} catch (e) {
			handleApiError(e);
		}
	};

	const {
		confirmId: unlockId,
		requestConfirm: requestUnlock,
		dialogProps,
	} = useConfirmDialog(handleForceUnlock);

	const handleCleanupExpired = async () => {
		try {
			const result = await adminLockService.cleanupExpired();
			toast.success(t("expired_locks_cleaned", { count: result.removed }));
			void reload();
		} catch (e) {
			handleApiError(e);
		}
	};

	const isExpired = (l: WebdavLock) =>
		l.timeout_at != null && new Date(l.timeout_at) < new Date();

	const unlockPath =
		unlockId !== null ? (locks.find((l) => l.id === unlockId)?.path ?? "") : "";

	return (
		<AdminLayout>
			<AdminPageShell>
				<AdminPageHeader
					title={t("webdav_locks")}
					description={t("locks_intro")}
					actions={
						<Button variant="outline" size="sm" onClick={handleCleanupExpired}>
							{t("clean_expired")}
						</Button>
					}
				/>
				<AdminTableList
					loading={loading}
					items={locks}
					columns={7}
					rows={6}
					emptyIcon={<Icon name="Lock" className="h-10 w-10" />}
					emptyTitle={t("no_active_locks")}
					emptyDescription={t("no_active_locks_desc")}
					headerRow={
						<TableHeader>
							<TableRow>
								<TableHead className="w-16">{t("id")}</TableHead>
								<TableHead>{t("path")}</TableHead>
								<TableHead>{t("common:owner")}</TableHead>
								<TableHead>{t("common:type")}</TableHead>
								<TableHead>{t("common:status")}</TableHead>
								<TableHead>{t("common:created_at")}</TableHead>
								<TableHead className={ADMIN_TABLE_ACTIONS_WIDTH_CLASS}>
									{t("common:actions")}
								</TableHead>
							</TableRow>
						</TableHeader>
					}
					renderRow={(l) => (
						<TableRow key={l.id}>
							<TableCell className="font-mono text-xs">{l.id}</TableCell>
							<TableCell className="font-mono text-xs max-w-[200px] truncate">
								{l.path}
							</TableCell>
							<TableCell className="text-xs">
								{l.owner_info ?? (l.owner_id != null ? `#${l.owner_id}` : "-")}
							</TableCell>
							<TableCell>
								<div className="flex gap-1">
									<Badge variant="outline">
										{l.shared ? t("shared_lock") : t("exclusive")}
									</Badge>
									{l.deep && <Badge variant="outline">{t("deep")}</Badge>}
								</div>
							</TableCell>
							<TableCell>
								{isExpired(l) ? (
									<StatusBadge status="expired" />
								) : (
									<StatusBadge status="active" />
								)}
							</TableCell>
							<TableCell className="text-muted-foreground text-xs">
								{formatDateShort(l.created_at)}
							</TableCell>
							<TableCell>
								<Button
									variant="ghost"
									size="icon"
									className={`${ADMIN_ICON_BUTTON_CLASS} text-destructive`}
									onClick={() => requestUnlock(l.id)}
								>
									<Icon name="Trash" className="h-3.5 w-3.5" />
								</Button>
							</TableCell>
						</TableRow>
					)}
				/>
			</AdminPageShell>

			<ConfirmDialog
				{...dialogProps}
				title={`Force unlock "${unlockPath}"?`}
				description={t("force_unlock_desc")}
				confirmLabel={t("common:confirm")}
				variant="destructive"
			/>
		</AdminLayout>
	);
}
