import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonFileGrid } from "@/components/common/SkeletonFileGrid";
import { SkeletonFileTable } from "@/components/common/SkeletonFileTable";
import { ViewToggle } from "@/components/common/ViewToggle";
import { AppLayout } from "@/components/layout/AppLayout";
import { TrashBatchActionBar } from "@/components/trash/TrashBatchActionBar";
import { TrashGrid } from "@/components/trash/TrashGrid";
import { TrashTable } from "@/components/trash/TrashTable";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { ItemCheckbox } from "@/components/ui/item-checkbox";
import { ScrollArea } from "@/components/ui/scroll-area";
import { STORAGE_KEYS } from "@/config/app";
import { handleApiError } from "@/hooks/useApiError";
import { useSelectionShortcuts } from "@/hooks/useSelectionShortcuts";
import { FOLDER_LIMIT } from "@/lib/constants";
import { formatBatchToast } from "@/lib/formatBatchToast";
import { trashService } from "@/services/trashService";
import { useAuthStore } from "@/stores/authStore";
import type { TrashContents, TrashItem } from "@/types/api";

type ViewMode = "grid" | "list";
type TrashOperation = "restore" | "purge";

function getStoredViewMode(): ViewMode {
	if (typeof window === "undefined") return "list";
	const stored = localStorage.getItem(STORAGE_KEYS.trashViewMode);
	return stored === "grid" ? "grid" : "list";
}

function getItemKey(item: TrashItem) {
	return `${item.entity_type}:${item.id}`;
}

function toTrashItems(contents: TrashContents): TrashItem[] {
	return [
		...contents.folders.map(
			(folder) =>
				({
					...folder,
					entity_type: "folder",
				}) as const,
		),
		...contents.files.map(
			(file) =>
				({
					...file,
					entity_type: "file",
				}) as const,
		),
	].sort(
		(a, b) =>
			new Date(b.deleted_at).getTime() - new Date(a.deleted_at).getTime(),
	);
}

export default function TrashPage() {
	const { t } = useTranslation(["common", "files", "admin"]);
	const refreshUser = useAuthStore((s) => s.refreshUser);
	const [contents, setContents] = useState<TrashContents>({
		files: [],
		folders: [],
		files_total: 0,
		folders_total: 0,
	});
	const [loading, setLoading] = useState(true);
	const [viewMode, setViewMode] = useState<ViewMode>(getStoredViewMode);
	const [selectedKeys, setSelectedKeys] = useState<Set<string>>(new Set());
	const [purgeTargets, setPurgeTargets] = useState<TrashItem[] | null>(null);
	const [purgeAllOpen, setPurgeAllOpen] = useState(false);
	const [loadingMore, setLoadingMore] = useState(false);
	const sentinelRef = useRef<HTMLDivElement | null>(null);

	const items = toTrashItems(contents);
	const hasMoreFiles = contents.next_file_cursor != null;
	const hasMoreFolders = contents.folders.length < contents.folders_total;
	const hasMore = hasMoreFiles || hasMoreFolders;
	const selectedItems = items.filter((item) =>
		selectedKeys.has(getItemKey(item)),
	);
	const selectionCount = selectedItems.length;
	const allSelected = items.length > 0 && selectionCount === items.length;
	const isEmpty = !loading && items.length === 0;

	const TRASH_PAGE_SIZE = 100;

	const load = useCallback(async () => {
		setLoading(true);
		try {
			const data = await trashService.list({
				folder_limit: FOLDER_LIMIT,
				file_limit: TRASH_PAGE_SIZE,
			});
			setContents(data);
			setSelectedKeys(new Set());
		} catch (err) {
			handleApiError(err);
		} finally {
			setLoading(false);
		}
	}, []);

	const loadMore = useCallback(async () => {
		if (loadingMore || !contents.next_file_cursor) return;
		setLoadingMore(true);
		try {
			const data = await trashService.list({
				folder_limit: 0,
				file_limit: TRASH_PAGE_SIZE,
				file_after_deleted_at: contents.next_file_cursor.deleted_at,
				file_after_id: contents.next_file_cursor.id,
			});
			setContents((prev) => ({
				...prev,
				files: [...prev.files, ...data.files],
				next_file_cursor: data.next_file_cursor,
			}));
		} catch (err) {
			handleApiError(err);
		} finally {
			setLoadingMore(false);
		}
	}, [contents.next_file_cursor, loadingMore]);

	useEffect(() => {
		void load();
	}, [load]);

	// Infinite scroll
	useEffect(() => {
		if (!hasMore || loadingMore) return;
		const el = sentinelRef.current;
		if (!el) return;
		const observer = new IntersectionObserver(
			(entries) => {
				if (entries[0].isIntersecting) void loadMore();
			},
			{ rootMargin: "200px" },
		);
		observer.observe(el);
		return () => observer.disconnect();
	}, [hasMore, loadingMore, loadMore]);

	const handleViewModeChange = (mode: ViewMode) => {
		localStorage.setItem(STORAGE_KEYS.trashViewMode, mode);
		setViewMode(mode);
	};

	const toggleSelect = (item: TrashItem) => {
		const key = getItemKey(item);
		setSelectedKeys((prev) => {
			const next = new Set(prev);
			if (next.has(key)) next.delete(key);
			else next.add(key);
			return next;
		});
	};

	const clearSelection = useCallback(() => {
		setSelectedKeys(new Set());
	}, []);

	const selectAllItems = useCallback(() => {
		setSelectedKeys(new Set(items.map(getItemKey)));
	}, [items]);

	const toggleSelectAll = useCallback(() => {
		if (allSelected) {
			clearSelection();
			return;
		}
		selectAllItems();
	}, [allSelected, clearSelection, selectAllItems]);

	useSelectionShortcuts({
		selectAll: selectAllItems,
		clearSelection,
		enabled: purgeTargets === null && !purgeAllOpen,
	});

	const runOperation = useCallback(
		async (targets: TrashItem[], operation: TrashOperation) => {
			if (targets.length === 0) return;

			const results = await Promise.allSettled(
				targets.map(async (item) => {
					if (operation === "restore") {
						if (item.entity_type === "file") {
							await trashService.restoreFile(item.id);
						} else {
							await trashService.restoreFolder(item.id);
						}
						return;
					}

					if (item.entity_type === "file") {
						await trashService.purgeFile(item.id);
					} else {
						await trashService.purgeFolder(item.id);
					}
				}),
			);

			const succeeded = results.filter(
				(result) => result.status === "fulfilled",
			).length;
			const failed = results.length - succeeded;

			const toastContent = formatBatchToast(t, operation, {
				succeeded,
				failed,
				errors: [],
			});
			if (toastContent.variant === "success") {
				toast.success(toastContent.title);
			} else {
				toast.error(toastContent.title);
			}

			if (succeeded > 0) {
				await Promise.all([load(), refreshUser()]);
			}
		},
		[load, refreshUser, t],
	);

	const handleRestore = useCallback(
		async (targets: TrashItem[]) => {
			try {
				await runOperation(targets, "restore");
			} catch (err) {
				handleApiError(err);
			}
		},
		[runOperation],
	);

	const handlePurge = useCallback(
		async (targets: TrashItem[]) => {
			try {
				await runOperation(targets, "purge");
			} catch (err) {
				handleApiError(err);
			}
		},
		[runOperation],
	);

	const handlePurgeAll = async () => {
		try {
			setPurgeAllOpen(false);
			await trashService.purgeAll();
			toast.success(t("common:trash_emptied"));
			await Promise.all([load(), refreshUser()]);
		} catch (err) {
			handleApiError(err);
		}
	};

	return (
		<AppLayout
			actions={<ViewToggle value={viewMode} onChange={handleViewModeChange} />}
		>
			<div className="flex flex-1 flex-col gap-4 overflow-hidden p-4">
				<div className="rounded-xl border bg-muted/20 px-4 py-4">
					<div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
						<div className="flex items-center gap-3">
							<div className="flex h-11 w-11 items-center justify-center rounded-xl bg-destructive/10 text-destructive">
								<Icon name="Trash" className="h-5 w-5" />
							</div>
							<div className="min-w-0">
								<h1 className="text-lg font-semibold">{t("common:trash")}</h1>
								<p className="text-sm text-muted-foreground">
									{t("files:trash_page_desc")}
								</p>
							</div>
						</div>
						{!isEmpty && !loading ? (
							<Button
								variant="destructive"
								size="sm"
								className="self-start"
								onClick={() => setPurgeAllOpen(true)}
							>
								<Icon name="Trash" className="mr-1 h-4 w-4" />
								{t("admin:empty_trash")}
							</Button>
						) : null}
					</div>
				</div>

				{!loading && !isEmpty ? (
					<div className="flex items-center justify-between rounded-xl border bg-background px-4 py-3">
						<div className="flex items-center gap-3">
							{viewMode === "grid" ? (
								<ItemCheckbox
									checked={allSelected}
									onChange={toggleSelectAll}
								/>
							) : null}
							<span className="text-sm font-medium">
								{selectionCount > 0
									? t("common:selected_count", { count: selectionCount })
									: t("common:items_count", { count: items.length })}
							</span>
						</div>
						<span className="hidden text-sm text-muted-foreground md:inline">
							{t("files:trash_page_desc")}
						</span>
					</div>
				) : null}

				<div className="min-h-0 flex flex-1 flex-col overflow-hidden rounded-xl border bg-background shadow-sm">
					{loading ? (
						viewMode === "grid" ? (
							<SkeletonFileGrid />
						) : (
							<SkeletonFileTable />
						)
					) : isEmpty ? (
						<EmptyState
							icon={<Icon name="Trash" className="h-10 w-10" />}
							title={t("files:trash_empty_title")}
							description={t("files:trash_empty_desc")}
						/>
					) : (
						<ScrollArea className="min-h-0 flex-1">
							{viewMode === "grid" ? (
								<TrashGrid
									items={items}
									selectedKeys={selectedKeys}
									onToggleSelect={toggleSelect}
									onRestore={(item) => {
										void handleRestore([item]);
									}}
									onPurge={(item) => setPurgeTargets([item])}
								/>
							) : (
								<TrashTable
									items={items}
									allSelected={allSelected}
									selectedKeys={selectedKeys}
									onToggleSelectAll={toggleSelectAll}
									onToggleSelect={toggleSelect}
									onRestore={(item) => {
										void handleRestore([item]);
									}}
									onPurge={(item) => setPurgeTargets([item])}
								/>
							)}
							{hasMore && (
								<div ref={sentinelRef} className="flex justify-center py-4">
									{loadingMore && (
										<div className="h-5 w-5 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-muted-foreground" />
									)}
								</div>
							)}
						</ScrollArea>
					)}
				</div>
			</div>

			<TrashBatchActionBar
				count={selectionCount}
				onRestore={() => {
					void handleRestore(selectedItems);
				}}
				onPurge={() => setPurgeTargets(selectedItems)}
				onClearSelection={clearSelection}
			/>

			<ConfirmDialog
				open={purgeTargets !== null}
				onOpenChange={(open) => {
					if (!open) setPurgeTargets(null);
				}}
				title={t("files:trash_purge_confirm_title", {
					count: purgeTargets?.length ?? 0,
				})}
				description={t("files:trash_purge_confirm_desc")}
				confirmLabel={t("files:trash_delete_permanently")}
				onConfirm={() => {
					const targets = purgeTargets;
					setPurgeTargets(null);
					if (targets) {
						void handlePurge(targets);
					}
				}}
				variant="destructive"
			/>

			<ConfirmDialog
				open={purgeAllOpen}
				onOpenChange={setPurgeAllOpen}
				title={t("common:are_you_sure")}
				description={t("admin:confirm_empty_trash")}
				confirmLabel={t("admin:empty_trash")}
				onConfirm={handlePurgeAll}
				variant="destructive"
			/>
		</AppLayout>
	);
}
