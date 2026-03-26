import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { AdminPageHeader } from "@/components/layout/AdminPageHeader";
import { AdminPageShell } from "@/components/layout/AdminPageShell";
import { AdminSurface } from "@/components/layout/AdminSurface";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { adminLockService } from "@/services/adminService";

interface WebdavLock {
	id: number;
	token: string;
	path: string;
	principal: string | null;
	owner_xml: string | null;
	timeout_at: string | null;
	shared: boolean;
	deep: boolean;
	created_at: string;
}

export default function AdminLocksPage() {
	const { t } = useTranslation("admin");
	const [locks, setLocks] = useState<WebdavLock[]>([]);
	const [loading, setLoading] = useState(true);
	const [unlockId, setUnlockId] = useState<number | null>(null);

	const load = useCallback(async () => {
		try {
			setLoading(true);
			const data = await adminLockService.list();
			setLocks(data);
		} catch (e) {
			handleApiError(e);
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		load();
	}, [load]);

	const handleForceUnlock = async (id: number) => {
		try {
			await adminLockService.forceUnlock(id);
			setLocks((prev) => prev.filter((l) => l.id !== id));
			toast.success(t("lock_released"));
		} catch (e) {
			handleApiError(e);
		}
	};

	const handleCleanupExpired = async () => {
		try {
			const result = await adminLockService.cleanupExpired();
			toast.success(t("expired_locks_cleaned", { count: result.removed }));
			load();
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
					description={t("no_active_locks_desc")}
					actions={
						<Button variant="outline" size="sm" onClick={handleCleanupExpired}>
							{t("clean_expired")}
						</Button>
					}
				/>
				{loading ? (
					<SkeletonTable columns={7} rows={6} />
				) : locks.length === 0 ? (
					<EmptyState
						icon={<Icon name="Lock" className="h-10 w-10" />}
						title={t("no_active_locks")}
						description={t("no_active_locks_desc")}
					/>
				) : (
					<AdminSurface>
						<ScrollArea className="min-h-0 flex-1">
							<Table>
								<TableHeader>
									<TableRow>
										<TableHead className="w-16">{t("id")}</TableHead>
										<TableHead>{t("path")}</TableHead>
										<TableHead>{t("principal")}</TableHead>
										<TableHead>{t("common:type")}</TableHead>
										<TableHead>{t("common:status")}</TableHead>
										<TableHead>{t("common:created_at")}</TableHead>
										<TableHead className="w-20">
											{t("common:actions")}
										</TableHead>
									</TableRow>
								</TableHeader>
								<TableBody>
									{locks.map((l) => (
										<TableRow key={l.id}>
											<TableCell className="font-mono text-xs">
												{l.id}
											</TableCell>
											<TableCell className="font-mono text-xs max-w-[200px] truncate">
												{l.path}
											</TableCell>
											<TableCell className="text-xs">
												{l.principal ?? "-"}
											</TableCell>
											<TableCell>
												<div className="flex gap-1">
													<Badge variant="outline">
														{l.shared ? t("shared_lock") : t("exclusive")}
													</Badge>
													{l.deep && (
														<Badge variant="outline">{t("deep")}</Badge>
													)}
												</div>
											</TableCell>
											<TableCell>
												{isExpired(l) ? (
													<Badge
														variant="outline"
														className="text-red-600 dark:text-red-400 border-red-600 dark:border-red-400"
													>
														{t("common:expired")}
													</Badge>
												) : (
													<Badge
														variant="outline"
														className="text-green-600 dark:text-green-400 border-green-600 dark:border-green-400"
													>
														{t("common:active")}
													</Badge>
												)}
											</TableCell>
											<TableCell className="text-muted-foreground text-xs">
												{new Date(l.created_at).toLocaleDateString()}
											</TableCell>
											<TableCell>
												<Button
													variant="ghost"
													size="icon"
													className="h-8 w-8 text-destructive"
													onClick={() => setUnlockId(l.id)}
												>
													<Icon name="Trash" className="h-3.5 w-3.5" />
												</Button>
											</TableCell>
										</TableRow>
									))}
								</TableBody>
							</Table>
						</ScrollArea>
					</AdminSurface>
				)}
			</AdminPageShell>

			<ConfirmDialog
				open={unlockId !== null}
				onOpenChange={(open) => {
					if (!open) setUnlockId(null);
				}}
				title={`Force unlock "${unlockPath}"?`}
				description={t("force_unlock_desc")}
				confirmLabel={t("common:confirm")}
				onConfirm={() => {
					const id = unlockId;
					setUnlockId(null);
					if (id !== null) void handleForceUnlock(id);
				}}
				variant="destructive"
			/>
		</AdminLayout>
	);
}
