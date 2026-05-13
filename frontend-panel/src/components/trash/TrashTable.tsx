import { useTranslation } from "react-i18next";
import { FileTypeIcon } from "@/components/files/FileTypeIcon";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { ItemCheckbox } from "@/components/ui/item-checkbox";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import {
	formatBytes,
	formatDateTimeWithOffset,
	formatDateUntil,
} from "@/lib/format";
import { cn } from "@/lib/utils";
import type { TrashItem } from "@/types/api-helpers";

interface TrashTableProps {
	items: TrashItem[];
	allSelected: boolean;
	actionsDisabled?: boolean;
	pendingKeys?: Set<string>;
	pendingOperation?: "restore" | "purge" | "purge-all" | null;
	selectedKeys: Set<string>;
	onToggleSelectAll: () => void;
	onToggleSelect: (item: TrashItem) => void;
	onRestore: (item: TrashItem) => void;
	onPurge: (item: TrashItem) => void;
}

function getItemKey(item: TrashItem) {
	return `${item.entity_type}:${item.id}`;
}

export function TrashTable({
	items,
	allSelected,
	actionsDisabled = false,
	pendingKeys = new Set<string>(),
	pendingOperation = null,
	selectedKeys,
	onToggleSelectAll,
	onToggleSelect,
	onRestore,
	onPurge,
}: TrashTableProps) {
	const { t, i18n } = useTranslation(["core", "files", "admin"]);

	return (
		<Table>
			<TableHeader>
				<TableRow>
					<TableHead className="w-12 pr-0 first:pl-3 md:first:pl-3">
						<div className="flex justify-center">
							<ItemCheckbox
								checked={allSelected}
								onChange={onToggleSelectAll}
							/>
						</div>
					</TableHead>
					<TableHead>{t("name")}</TableHead>
					<TableHead>{t("original_location")}</TableHead>
					<TableHead className="w-[180px]">
						{t("files:trash_expires_at")}
					</TableHead>
					<TableHead className="w-[100px]">{t("size")}</TableHead>
					<TableHead className="w-[180px] text-right">{t("actions")}</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{items.map((item) => {
					const itemKey = getItemKey(item);
					const selected = selectedKeys.has(itemKey);
					const itemPending = pendingKeys.has(itemKey);
					const restorePending = itemPending && pendingOperation === "restore";
					const purgePending = itemPending && pendingOperation === "purge";
					const restoreLabel = restorePending
						? t("files:trash_restoring")
						: t("admin:restore");
					const purgeLabel = purgePending
						? t("files:trash_purging")
						: t("files:trash_delete_permanently");
					const originalPath =
						item.original_path === "/" ? t("files:root") : item.original_path;

					return (
						<TableRow
							key={itemKey}
							className={cn(
								"cursor-pointer transition-colors",
								selected && "bg-accent/40",
								itemPending && "opacity-75",
							)}
							onClick={() => {
								if (!itemPending) onToggleSelect(item);
							}}
						>
							<TableCell
								className="w-12 pr-0 first:pl-3 md:first:pl-3"
								onClick={(e) => e.stopPropagation()}
							>
								<div className="flex justify-center">
									<ItemCheckbox
										checked={selected}
										onChange={() => {
											if (!itemPending) onToggleSelect(item);
										}}
									/>
								</div>
							</TableCell>
							<TableCell>
								<div className="flex items-center gap-2">
									{item.entity_type === "folder" ? (
										<Icon name="Folder" className="h-4 w-4 text-amber-500" />
									) : (
										<FileTypeIcon
											mimeType={item.mime_type}
											fileName={item.name}
											className="h-4 w-4"
										/>
									)}
									<div className="min-w-0">
										<p className="truncate font-medium" title={item.name}>
											{item.name}
										</p>
										<p className="text-xs text-muted-foreground">
											{item.entity_type === "folder" ? t("folder") : t("file")}
										</p>
									</div>
								</div>
							</TableCell>
							<TableCell
								className="max-w-[280px] truncate text-muted-foreground"
								title={originalPath}
							>
								{originalPath}
							</TableCell>
							<TableCell title={formatDateTimeWithOffset(item.expires_at)}>
								{formatDateUntil(item.expires_at, i18n)}
							</TableCell>
							<TableCell>
								{item.entity_type === "file" ? formatBytes(item.size) : "—"}
							</TableCell>
							<TableCell className="text-right">
								<div className="flex justify-end gap-1">
									<Button
										size="icon-sm"
										variant="ghost"
										aria-label={restoreLabel}
										disabled={actionsDisabled || itemPending}
										onClick={(e) => {
											e.stopPropagation();
											onRestore(item);
										}}
										title={restoreLabel}
									>
										<Icon
											name={
												restorePending ? "Spinner" : "ArrowCounterClockwise"
											}
											className={`h-4 w-4 ${restorePending ? "animate-spin" : ""}`}
										/>
									</Button>
									<Button
										size="icon-sm"
										variant="ghost"
										className="text-destructive hover:text-destructive"
										aria-label={purgeLabel}
										disabled={actionsDisabled || itemPending}
										onClick={(e) => {
											e.stopPropagation();
											onPurge(item);
										}}
										title={purgeLabel}
									>
										<Icon
											name={purgePending ? "Spinner" : "Trash"}
											className={`h-4 w-4 ${purgePending ? "animate-spin" : ""}`}
										/>
									</Button>
								</div>
							</TableCell>
						</TableRow>
					);
				})}
			</TableBody>
		</Table>
	);
}
