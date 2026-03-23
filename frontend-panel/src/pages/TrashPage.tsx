import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AppLayout } from "@/components/layout/AppLayout";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { formatDate } from "@/lib/format";
import { trashService } from "@/services/trashService";
import type { FileInfo, FolderInfo } from "@/types/api";

export default function TrashPage() {
	const { t } = useTranslation();
	const [files, setFiles] = useState<FileInfo[]>([]);
	const [folders, setFolders] = useState<FolderInfo[]>([]);
	const [loading, setLoading] = useState(true);
	const [purgeAllOpen, setPurgeAllOpen] = useState(false);
	const [purgeTarget, setPurgeTarget] = useState<{
		type: "file" | "folder";
		id: number;
	} | null>(null);

	const load = useCallback(async () => {
		try {
			const data = await trashService.list();
			setFiles(data.files);
			setFolders(data.folders);
		} catch (err) {
			handleApiError(err);
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		load();
	}, [load]);

	const handleRestore = async (type: "file" | "folder", id: number) => {
		try {
			if (type === "file") await trashService.restoreFile(id);
			else await trashService.restoreFolder(id);
			toast.success(t("restored"));
			load();
		} catch (err) {
			handleApiError(err);
		}
	};

	const handlePurge = async (type: "file" | "folder", id: number) => {
		try {
			if (type === "file") await trashService.purgeFile(id);
			else await trashService.purgeFolder(id);
			toast.success(t("permanently_deleted"));
			load();
		} catch (err) {
			handleApiError(err);
		}
	};

	const handlePurgeAll = async () => {
		try {
			await trashService.purgeAll();
			toast.success(t("trash_emptied"));
			load();
		} catch (err) {
			handleApiError(err);
		}
	};

	const isEmpty = files.length === 0 && folders.length === 0;

	return (
		<AppLayout
			actions={
				!isEmpty && !loading ? (
					<Button
						variant="destructive"
						size="sm"
						onClick={() => setPurgeAllOpen(true)}
					>
						<Icon name="Trash" className="h-4 w-4 mr-1" />
						{t("admin:empty_trash")}
					</Button>
				) : undefined
			}
		>
			<div className="flex-1 overflow-auto p-6">
				{loading ? (
					<SkeletonTable columns={3} rows={8} />
				) : isEmpty ? (
					<EmptyState
						icon={<Icon name="Trash" className="h-10 w-10" />}
						title={t("admin:trash_empty")}
						description={t("admin:trash_empty_desc")}
					/>
				) : (
					<Table>
						<TableHeader>
							<TableRow>
								<TableHead className="w-[50%]">{t("name")}</TableHead>
								<TableHead>{t("admin:deleted_at")}</TableHead>
								<TableHead className="w-[120px]">{t("actions")}</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{folders.map((f) => (
								<TableRow key={`folder-${f.id}`}>
									<TableCell className="flex items-center gap-2">
										<Icon name="Folder" className="h-4 w-4 text-blue-500" />
										{f.name}
									</TableCell>
									<TableCell className="text-muted-foreground">
										{f.deleted_at ? formatDate(f.deleted_at) : "-"}
									</TableCell>
									<TableCell>
										<div className="flex gap-1">
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8"
												title={t("admin:restore")}
												onClick={() => handleRestore("folder", f.id)}
											>
												<Icon
													name="ArrowCounterClockwise"
													className="h-4 w-4"
												/>
											</Button>
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8 text-destructive"
												title={t("admin:purge")}
												onClick={() =>
													setPurgeTarget({
														type: "folder",
														id: f.id,
													})
												}
											>
												<Icon name="Trash" className="h-4 w-4" />
											</Button>
										</div>
									</TableCell>
								</TableRow>
							))}
							{files.map((f) => (
								<TableRow key={`file-${f.id}`}>
									<TableCell className="flex items-center gap-2">
										<Icon
											name="File"
											className="h-4 w-4 text-muted-foreground"
										/>
										{f.name}
									</TableCell>
									<TableCell className="text-muted-foreground">
										{f.deleted_at ? formatDate(f.deleted_at) : "-"}
									</TableCell>
									<TableCell>
										<div className="flex gap-1">
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8"
												title={t("admin:restore")}
												onClick={() => handleRestore("file", f.id)}
											>
												<Icon
													name="ArrowCounterClockwise"
													className="h-4 w-4"
												/>
											</Button>
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8 text-destructive"
												title={t("admin:purge")}
												onClick={() =>
													setPurgeTarget({
														type: "file",
														id: f.id,
													})
												}
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
			</div>

			<ConfirmDialog
				open={purgeAllOpen}
				onOpenChange={setPurgeAllOpen}
				title={t("are_you_sure")}
				description={t("admin:confirm_empty_trash")}
				confirmLabel={t("admin:empty_trash")}
				onConfirm={handlePurgeAll}
				variant="destructive"
			/>

			<ConfirmDialog
				open={purgeTarget !== null}
				onOpenChange={(open) => {
					if (!open) setPurgeTarget(null);
				}}
				title={t("are_you_sure")}
				description={t("admin:confirm_purge")}
				confirmLabel={t("admin:purge")}
				onConfirm={() => {
					if (purgeTarget) {
						handlePurge(purgeTarget.type, purgeTarget.id);
					}
					setPurgeTarget(null);
				}}
				variant="destructive"
			/>
		</AppLayout>
	);
}
