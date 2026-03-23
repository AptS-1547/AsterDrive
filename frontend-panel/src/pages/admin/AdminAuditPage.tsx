import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
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
import { formatDateAbsolute } from "@/lib/format";
import { auditService } from "@/services/auditService";
import type { AuditLogEntry } from "@/types/api";

export default function AdminAuditPage() {
	const { t } = useTranslation("admin");
	const [items, setItems] = useState<AuditLogEntry[]>([]);
	const [total, setTotal] = useState(0);
	const [offset, setOffset] = useState(0);
	const [actionFilter, setActionFilter] = useState("");
	const [entityTypeFilter, setEntityTypeFilter] = useState("__all__");
	const [loading, setLoading] = useState(true);
	const limit = 20;

	const load = useCallback(async () => {
		setLoading(true);
		try {
			const page = await auditService.list({
				action: actionFilter || undefined,
				entity_type:
					entityTypeFilter === "__all__" ? undefined : entityTypeFilter,
				limit,
				offset,
			});
			setItems(page.items);
			setTotal(page.total);
		} catch (err) {
			handleApiError(err);
		} finally {
			setLoading(false);
		}
	}, [offset, actionFilter, entityTypeFilter]);

	useEffect(() => {
		load();
	}, [load]);

	const totalPages = Math.ceil(total / limit);
	const currentPage = Math.floor(offset / limit) + 1;

	return (
		<AdminLayout>
			<div className="p-4 space-y-4">
				<h2 className="text-lg font-semibold">{t("audit_log")}</h2>
				<div className="flex gap-2">
					<Input
						placeholder={t("audit_filter_action")}
						value={actionFilter}
						onChange={(e) => {
							setActionFilter(e.target.value);
							setOffset(0);
						}}
						className="max-w-xs"
					/>
					<Select
						value={entityTypeFilter}
						onValueChange={(v) => {
							if (v) setEntityTypeFilter(v);
							setOffset(0);
						}}
					>
						<SelectTrigger className="w-[150px]">
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="__all__">{t("audit_all_types")}</SelectItem>
							<SelectItem value="file">{t("common:file")}</SelectItem>
							<SelectItem value="folder">{t("common:folder")}</SelectItem>
						</SelectContent>
					</Select>
				</div>

				{loading ? (
					<SkeletonTable columns={6} rows={8} />
				) : items.length === 0 ? (
					<EmptyState
						icon={<Icon name="Scroll" className="h-10 w-10" />}
						title={t("no_audit_logs")}
					/>
				) : (
					<ScrollArea className="flex-1">
						<Table>
							<TableHeader>
								<TableRow>
									<TableHead>{t("audit_time")}</TableHead>
									<TableHead>{t("audit_user")}</TableHead>
									<TableHead>{t("audit_action")}</TableHead>
									<TableHead>{t("audit_entity")}</TableHead>
									<TableHead>{t("common:name")}</TableHead>
									<TableHead>{t("audit_ip")}</TableHead>
								</TableRow>
							</TableHeader>
							<TableBody>
								{items.map((item) => (
									<TableRow key={item.id}>
										<TableCell className="text-xs text-muted-foreground whitespace-nowrap">
											{formatDateAbsolute(item.created_at)}
										</TableCell>
										<TableCell>{item.user_id}</TableCell>
										<TableCell>
											<span className="inline-flex items-center rounded-full bg-blue-50 dark:bg-blue-950 px-2 py-0.5 text-xs font-medium text-blue-700 dark:text-blue-300">
												{item.action}
											</span>
										</TableCell>
										<TableCell className="text-muted-foreground">
											{item.entity_type ?? "---"}
										</TableCell>
										<TableCell>{item.entity_name ?? "---"}</TableCell>
										<TableCell className="text-xs text-muted-foreground">
											{item.ip_address ?? "---"}
										</TableCell>
									</TableRow>
								))}
							</TableBody>
						</Table>
					</ScrollArea>
				)}

				{totalPages > 1 && (
					<div className="flex items-center justify-between">
						<span className="text-sm text-muted-foreground">
							{t("entries_page", {
								total,
								current: currentPage,
								pages: totalPages,
							})}
						</span>
						<div className="flex gap-1">
							<Button
								variant="outline"
								size="sm"
								disabled={offset === 0}
								onClick={() => setOffset(Math.max(0, offset - limit))}
							>
								<Icon name="CaretLeft" className="h-4 w-4" />
							</Button>
							<Button
								variant="outline"
								size="sm"
								disabled={offset + limit >= total}
								onClick={() => setOffset(offset + limit)}
							>
								<Icon name="CaretRight" className="h-4 w-4" />
							</Button>
						</div>
					</div>
				)}
			</div>
		</AdminLayout>
	);
}
