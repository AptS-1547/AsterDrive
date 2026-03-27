import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { AdminTableList } from "@/components/common/AdminTableList";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
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
import { adminShareService } from "@/services/adminService";
import type { ShareInfo } from "@/types/api";

export default function AdminSharesPage() {
	const { t } = useTranslation("admin");
	const {
		items: shares,
		setItems: setShares,
		loading,
	} = useApiList(() => adminShareService.list({ limit: 100, offset: 0 }));

	const handleDelete = async (id: number) => {
		try {
			await adminShareService.delete(id);
			setShares((prev) => prev.filter((s) => s.id !== id));
			toast.success(t("share_deleted"));
		} catch (e) {
			handleApiError(e);
		}
	};

	const {
		confirmId: deleteId,
		requestConfirm,
		dialogProps,
	} = useConfirmDialog(handleDelete);

	const isExpired = (s: ShareInfo) =>
		s.expires_at != null && new Date(s.expires_at) < new Date();

	const isLimitReached = (s: ShareInfo) =>
		s.max_downloads > 0 && s.download_count >= s.max_downloads;

	const deleteToken =
		deleteId !== null
			? (shares.find((s) => s.id === deleteId)?.token ?? "")
			: "";

	return (
		<AdminLayout>
			<AdminPageShell>
				<AdminPageHeader title={t("shares")} description={t("shares_intro")} />
				<AdminTableList
					loading={loading}
					items={shares}
					columns={8}
					rows={6}
					emptyIcon={<Icon name="LinkSimple" className="h-10 w-10" />}
					emptyTitle={t("no_shares")}
					emptyDescription={t("no_shares_desc")}
					headerRow={
						<TableHeader>
							<TableRow>
								<TableHead className="w-16">{t("id")}</TableHead>
								<TableHead>Token</TableHead>
								<TableHead>{t("audit_user")}</TableHead>
								<TableHead>{t("common:type")}</TableHead>
								<TableHead>{t("common:status")}</TableHead>
								<TableHead>Downloads</TableHead>
								<TableHead>{t("common:created_at")}</TableHead>
								<TableHead className={ADMIN_TABLE_ACTIONS_WIDTH_CLASS}>
									{t("common:actions")}
								</TableHead>
							</TableRow>
						</TableHeader>
					}
					renderRow={(s) => (
						<TableRow key={s.id}>
							<TableCell className="font-mono text-xs">{s.id}</TableCell>
							<TableCell>
								<a
									href={`/s/${s.token}`}
									target="_blank"
									rel="noreferrer"
									className="font-mono text-xs text-primary hover:underline inline-flex items-center gap-1"
								>
									{s.token}
									<Icon name="ArrowSquareOut" className="h-3 w-3" />
								</a>
							</TableCell>
							<TableCell className="text-xs">#{s.user_id}</TableCell>
							<TableCell>
								<Badge variant="outline">
									{s.file_id != null ? t("common:file") : t("common:folder")}
								</Badge>
							</TableCell>
							<TableCell>
								{isExpired(s) ? (
									<Badge
										variant="outline"
										className="text-red-600 dark:text-red-400 border-red-600 dark:border-red-400"
									>
										{t("common:expired")}
									</Badge>
								) : isLimitReached(s) ? (
									<Badge
										variant="outline"
										className="text-orange-600 dark:text-orange-400 border-orange-600 dark:border-orange-400"
									>
										{t("limit_reached")}
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
							<TableCell className="text-xs">
								{s.download_count}
								{s.max_downloads > 0 ? ` / ${s.max_downloads}` : ""}
							</TableCell>
							<TableCell className="text-muted-foreground text-xs">
								{formatDateShort(s.created_at)}
							</TableCell>
							<TableCell>
								<Button
									variant="ghost"
									size="icon"
									className={`${ADMIN_ICON_BUTTON_CLASS} text-destructive`}
									onClick={() => requestConfirm(s.id)}
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
				title={`${t("common:delete")} "${deleteToken}"?`}
				description={t("delete_share_desc")}
				confirmLabel={t("common:delete")}
				variant="destructive"
			/>
		</AdminLayout>
	);
}
