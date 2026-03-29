import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { EmptyState } from "@/components/common/EmptyState";
import { FileTypeIcon } from "@/components/files/FileTypeIcon";
import { AppLayout } from "@/components/layout/AppLayout";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Icon } from "@/components/ui/icon";
import { handleApiError } from "@/hooks/useApiError";
import { formatDateAbsolute } from "@/lib/format";
import { cn } from "@/lib/utils";
import { shareService } from "@/services/shareService";
import type { MyShareInfo, ShareStatus } from "@/types/api";

const PAGE_SIZE = 50;

export default function MySharesPage() {
	const { t } = useTranslation(["core", "share"]);
	const [page, setPage] = useState(0);
	const [loading, setLoading] = useState(true);
	const [shares, setShares] = useState<MyShareInfo[]>([]);
	const [total, setTotal] = useState(0);
	const [deleteTarget, setDeleteTarget] = useState<MyShareInfo | null>(null);

	const loadShares = useCallback(async () => {
		try {
			setLoading(true);
			const data = await shareService.listMine({
				limit: PAGE_SIZE,
				offset: page * PAGE_SIZE,
			});
			setShares(data.items);
			setTotal(data.total);
		} catch (error) {
			handleApiError(error);
		} finally {
			setLoading(false);
		}
	}, [page]);

	useEffect(() => {
		void loadShares().catch(() => {});
	}, [loadShares]);

	const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

	const copyShareLink = async (share: MyShareInfo) => {
		const url = `${window.location.origin}/s/${share.token}`;
		await navigator.clipboard.writeText(url);
		toast.success(t("copied_to_clipboard"));
	};

	const handleDelete = async () => {
		if (!deleteTarget) return;
		try {
			await shareService.delete(deleteTarget.id);
			toast.success(t("share:my_shares_delete_success"));
			setDeleteTarget(null);
			if (shares.length === 1 && page > 0) {
				setPage((current) => current - 1);
				return;
			}
			await loadShares();
		} catch (error) {
			handleApiError(error);
		}
	};

	const statusBadge = (status: ShareStatus) => {
		switch (status) {
			case "active":
				return <Badge variant="secondary">{t("active")}</Badge>;
			case "expired":
				return <Badge variant="outline">{t("expired")}</Badge>;
			case "exhausted":
				return (
					<Badge variant="outline">
						{t("share:my_shares_status_exhausted")}
					</Badge>
				);
			case "deleted":
				return (
					<Badge variant="destructive">
						{t("share:my_shares_status_deleted")}
					</Badge>
				);
		}
	};

	return (
		<AppLayout>
			<div className="min-h-0 flex-1 overflow-auto">
				<div className="mx-auto flex w-full max-w-7xl flex-col gap-5 p-4 md:p-6">
					<div className="flex items-center gap-3">
						<h1 className="text-2xl font-semibold tracking-tight">
							{t("share:my_shares_title")}
						</h1>
						<Button
							variant="ghost"
							size="icon-sm"
							onClick={() => void loadShares()}
							disabled={loading}
							aria-label={t("refresh")}
							title={t("refresh")}
						>
							<Icon
								name={loading ? "Spinner" : "ArrowsClockwise"}
								className={`h-4 w-4 ${loading ? "animate-spin" : ""}`}
							/>
						</Button>
					</div>

					{loading ? (
						<div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
							{["s1", "s2", "s3", "s4", "s5", "s6"].map((key) => (
								<Card key={key} className="h-28 animate-pulse bg-muted/20" />
							))}
						</div>
					) : shares.length === 0 ? (
						<Card className="bg-muted/15">
							<div className="py-12">
								<EmptyState
									icon={<Icon name="Link" className="h-10 w-10" />}
									title={t("share:my_shares_empty_title")}
									description={t("share:my_shares_empty_desc")}
								/>
							</div>
						</Card>
					) : (
						<>
							<div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
								{shares.map((share) => {
									const isFolder = share.resource_type === "folder";

									return (
										<ContextMenu key={share.id}>
											<ContextMenuTrigger className="w-full">
												<Card
													className={cn(
														"cursor-pointer border bg-background px-4 py-3 shadow-sm transition-all duration-150 hover:-translate-y-0.5 hover:bg-muted/5 hover:shadow-md",
													)}
													onClick={() =>
														window.open(
															`/s/${share.token}`,
															"_blank",
															"noopener,noreferrer",
														)
													}
												>
													<div className="flex items-center gap-3">
														<div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-muted/45">
															{isFolder ? (
																<Icon
																	name="Folder"
																	className="h-5 w-5 text-amber-500"
																/>
															) : (
																<FileTypeIcon
																	mimeType=""
																	fileName={share.resource_name}
																	className="h-5 w-5"
																/>
															)}
														</div>
														<div className="min-w-0 flex-1">
															<span className="block truncate text-sm font-semibold">
																{share.resource_name}
															</span>
														</div>
														<div className="shrink-0">
															{statusBadge(share.status)}
														</div>
													</div>

													<div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 pl-[52px] text-xs text-muted-foreground">
														<span>
															{t("share:my_shares_created_label", {
																date: formatDateAbsolute(share.created_at),
															})}
														</span>
														{share.expires_at ? (
															<span>
																{t("share:my_shares_expire_label", {
																	date: formatDateAbsolute(share.expires_at),
																})}
															</span>
														) : (
															<span>{t("share:my_shares_never")}</span>
														)}
														{share.has_password && (
															<Icon name="Lock" className="h-3 w-3" />
														)}
													</div>
												</Card>
											</ContextMenuTrigger>
											<ContextMenuContent>
												<ContextMenuItem
													onClick={() => void copyShareLink(share)}
												>
													<Icon name="Copy" />
													{t("share:my_shares_card_copy")}
												</ContextMenuItem>
												<ContextMenuItem
													onClick={() =>
														window.open(
															`/s/${share.token}`,
															"_blank",
															"noopener,noreferrer",
														)
													}
												>
													<Icon name="ArrowSquareOut" />
													{t("share:my_shares_card_open")}
												</ContextMenuItem>
												<ContextMenuSeparator />
												<ContextMenuItem
													variant="destructive"
													onClick={() => setDeleteTarget(share)}
												>
													<Icon name="Trash" />
													{t("share:my_shares_card_delete")}
												</ContextMenuItem>
											</ContextMenuContent>
										</ContextMenu>
									);
								})}
							</div>

							<div className="flex items-center justify-between rounded-xl border bg-muted/15 px-4 py-3">
								<p className="text-sm text-muted-foreground">
									{t("share:my_shares_pagination_desc", {
										current: page + 1,
										total: totalPages,
										count: total,
									})}
								</p>
								<div className="flex items-center gap-2">
									<Button
										variant="outline"
										size="sm"
										disabled={page === 0}
										onClick={() =>
											setPage((current) => Math.max(0, current - 1))
										}
									>
										{t("share:my_shares_prev")}
									</Button>
									<Button
										variant="outline"
										size="sm"
										disabled={page + 1 >= totalPages}
										onClick={() =>
											setPage((current) =>
												current + 1 >= totalPages ? current : current + 1,
											)
										}
									>
										{t("share:my_shares_next")}
									</Button>
								</div>
							</div>
						</>
					)}
				</div>
			</div>

			<ConfirmDialog
				open={deleteTarget !== null}
				onOpenChange={(open) => {
					if (!open) setDeleteTarget(null);
				}}
				title={t("share:my_shares_delete_title", {
					name: deleteTarget?.resource_name ?? "",
				})}
				description={t("share:my_shares_delete_desc")}
				confirmLabel={t("delete")}
				onConfirm={() => void handleDelete()}
				variant="destructive"
			/>
		</AppLayout>
	);
}
