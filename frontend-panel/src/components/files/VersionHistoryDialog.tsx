import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { FileTypeIcon } from "@/components/files/FileTypeIcon";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
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
import { invalidateBlobUrl } from "@/hooks/useBlobUrl";
import { invalidateTextContent } from "@/hooks/useTextContent";
import { formatBytes } from "@/lib/format";
import type { FileVersion } from "@/services/fileService";
import { fileService } from "@/services/fileService";

interface VersionHistoryDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	fileId: number;
	fileName: string;
	mimeType?: string;
	currentSize?: number;
	onRestored?: () => void;
}

export function VersionHistoryDialog({
	open,
	onOpenChange,
	fileId,
	fileName,
	mimeType,
	currentSize,
	onRestored,
}: VersionHistoryDialogProps) {
	const { t } = useTranslation("files");
	const [versions, setVersions] = useState<FileVersion[]>([]);
	const [loading, setLoading] = useState(false);
	const [restoringVersionId, setRestoringVersionId] = useState<number | null>(
		null,
	);
	const [deletingVersionId, setDeletingVersionId] = useState<number | null>(
		null,
	);
	const [confirmRestoreVersion, setConfirmRestoreVersion] =
		useState<FileVersion | null>(null);
	const [confirmDeleteVersion, setConfirmDeleteVersion] =
		useState<FileVersion | null>(null);

	const load = useCallback(async () => {
		try {
			setLoading(true);
			const data = await fileService.listVersions(fileId);
			setVersions(data);
		} catch (e) {
			handleApiError(e);
		} finally {
			setLoading(false);
		}
	}, [fileId]);

	useEffect(() => {
		if (open) {
			load();
			return;
		}

		setVersions([]);
		setLoading(false);
		setRestoringVersionId(null);
		setDeletingVersionId(null);
		setConfirmRestoreVersion(null);
		setConfirmDeleteVersion(null);
	}, [open, load]);

	const handleRestore = async (versionId: number) => {
		try {
			setRestoringVersionId(versionId);
			await fileService.restoreVersion(fileId, versionId);
			const downloadPath = fileService.downloadPath(fileId);
			invalidateTextContent(downloadPath);
			invalidateBlobUrl(downloadPath);
			invalidateBlobUrl(fileService.thumbnailPath(fileId));
			toast.success(t("version_restored"));
			onRestored?.();
		} catch (e) {
			handleApiError(e);
		} finally {
			setRestoringVersionId(null);
		}
	};

	const handleDelete = async (versionId: number) => {
		try {
			setDeletingVersionId(versionId);
			await fileService.deleteVersion(fileId, versionId);
			toast.success(t("version_deleted"));
			setVersions((prev) => prev.filter((v) => v.id !== versionId));
		} catch (e) {
			handleApiError(e);
		} finally {
			setDeletingVersionId(null);
		}
	};

	return (
		<>
			<Dialog open={open} onOpenChange={onOpenChange}>
				<DialogContent className="max-w-lg">
					<DialogHeader>
						<div className="flex items-start gap-3 pr-8">
							{mimeType ? (
								<FileTypeIcon
									mimeType={mimeType}
									fileName={fileName}
									className="mt-0.5 h-5 w-5 shrink-0"
								/>
							) : null}
							<div className="min-w-0">
								<DialogTitle>
									{t("version_history_title", { name: fileName })}
								</DialogTitle>
								{(mimeType || currentSize !== undefined) && (
									<div className="mt-1 text-xs text-muted-foreground">
										{mimeType ?? t("common:file")}
										{currentSize !== undefined
											? ` · ${formatBytes(currentSize)}`
											: ""}
									</div>
								)}
							</div>
						</div>
					</DialogHeader>
					<div className="mb-4 rounded-lg border bg-muted/20 p-3">
						<div className="flex items-center gap-3">
							{mimeType ? (
								<FileTypeIcon
									mimeType={mimeType}
									fileName={fileName}
									className="h-5 w-5 shrink-0"
								/>
							) : null}
							<div className="min-w-0 flex-1">
								<div className="text-sm font-medium text-foreground">
									{t("version_current")}
								</div>
								<div className="mt-1 text-xs text-muted-foreground">
									{currentSize !== undefined
										? formatBytes(currentSize)
										: t("common:file")}
									{mimeType ? ` · ${mimeType}` : ""}
								</div>
							</div>
							<div className="text-xs text-muted-foreground">
								{t("version_history_count", { count: versions.length })}
							</div>
						</div>
					</div>
					{loading ? (
						<p className="text-muted-foreground text-sm py-4 text-center">
							{t("loading_preview")}
						</p>
					) : versions.length === 0 ? (
						<p className="text-muted-foreground text-sm py-4 text-center">
							{t("version_empty")}
						</p>
					) : (
						<Table>
							<TableHeader>
								<TableRow>
									<TableHead>{t("version_column")}</TableHead>
									<TableHead>{t("version_size")}</TableHead>
									<TableHead>{t("version_date")}</TableHead>
									<TableHead className="w-20">{t("version_actions")}</TableHead>
								</TableRow>
							</TableHeader>
							<TableBody>
								{versions.map((v) => (
									<TableRow key={v.id}>
										<TableCell className="font-mono text-sm">
											v{v.version}
										</TableCell>
										<TableCell className="text-sm">
											{formatBytes(v.size)}
										</TableCell>
										<TableCell className="text-muted-foreground text-xs">
											{new Date(v.created_at).toLocaleString()}
										</TableCell>
										<TableCell>
											<div className="flex gap-1">
												<Button
													variant="ghost"
													size="icon"
													className="h-7 w-7"
													title={
														restoringVersionId === v.id
															? t("version_restoring")
															: t("version_restore")
													}
													disabled={
														restoringVersionId !== null ||
														deletingVersionId !== null
													}
													onClick={() => setConfirmRestoreVersion(v)}
												>
													<Icon
														name={
															restoringVersionId === v.id
																? "Spinner"
																: "ArrowCounterClockwise"
														}
														className={`h-3.5 w-3.5 ${restoringVersionId === v.id ? "animate-spin" : ""}`}
													/>
												</Button>
												<Button
													variant="ghost"
													size="icon"
													className="h-7 w-7 text-destructive"
													title={
														deletingVersionId === v.id
															? t("version_deleting")
															: t("version_delete")
													}
													disabled={
														restoringVersionId !== null ||
														deletingVersionId !== null
													}
													onClick={() => setConfirmDeleteVersion(v)}
												>
													<Icon
														name={
															deletingVersionId === v.id ? "Spinner" : "Trash"
														}
														className={`h-3.5 w-3.5 ${deletingVersionId === v.id ? "animate-spin" : ""}`}
													/>
												</Button>
											</div>
										</TableCell>
									</TableRow>
								))}
							</TableBody>
						</Table>
					)}
				</DialogContent>
			</Dialog>
			<ConfirmDialog
				open={confirmRestoreVersion !== null}
				onOpenChange={(nextOpen) => {
					if (!nextOpen) setConfirmRestoreVersion(null);
				}}
				title={t("version_restore_confirm_title")}
				description={t("version_restore_confirm_desc", {
					version: confirmRestoreVersion?.version,
				})}
				confirmLabel={t("version_restore")}
				onConfirm={() => {
					if (!confirmRestoreVersion) return;
					handleRestore(confirmRestoreVersion.id);
				}}
			/>
			<ConfirmDialog
				open={confirmDeleteVersion !== null}
				onOpenChange={(nextOpen) => {
					if (!nextOpen) setConfirmDeleteVersion(null);
				}}
				title={t("version_delete_confirm_title")}
				description={t("version_delete_confirm_desc", {
					version: confirmDeleteVersion?.version,
				})}
				confirmLabel={t("version_delete")}
				variant="destructive"
				onConfirm={() => {
					if (!confirmDeleteVersion) return;
					handleDelete(confirmDeleteVersion.id);
				}}
			/>
		</>
	);
}
