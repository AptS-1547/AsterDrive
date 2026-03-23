import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
	DialogTrigger,
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
import type { FileVersion } from "@/services/fileService";
import { fileService } from "@/services/fileService";

function formatSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
	if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
	return `${(bytes / 1073741824).toFixed(1)} GB`;
}

interface VersionHistoryDialogProps {
	fileId: number;
	fileName: string;
	onRestored?: () => void;
}

export function VersionHistoryDialog({
	fileId,
	fileName,
	onRestored,
}: VersionHistoryDialogProps) {
	const { t } = useTranslation("files");
	const [open, setOpen] = useState(false);
	const [versions, setVersions] = useState<FileVersion[]>([]);
	const [loading, setLoading] = useState(false);

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
		if (open) load();
	}, [open, load]);

	const handleRestore = async (versionId: number) => {
		try {
			await fileService.restoreVersion(fileId, versionId);
			toast.success(t("version_restored"));
			load();
			onRestored?.();
		} catch (e) {
			handleApiError(e);
		}
	};

	const handleDelete = async (versionId: number) => {
		try {
			await fileService.deleteVersion(fileId, versionId);
			toast.success(t("version_deleted"));
			setVersions((prev) => prev.filter((v) => v.id !== versionId));
		} catch (e) {
			handleApiError(e);
		}
	};

	return (
		<Dialog open={open} onOpenChange={setOpen}>
			<DialogTrigger
				render={<Button variant="ghost" size="icon" className="h-8 w-8" />}
			>
				<Icon name="Clock" className="h-4 w-4" />
			</DialogTrigger>
			<DialogContent className="max-w-lg">
				<DialogHeader>
					<DialogTitle>
						{t("version_history_title", { name: fileName })}
					</DialogTitle>
				</DialogHeader>
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
										{formatSize(v.size)}
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
												title={t("version_restore")}
												onClick={() => handleRestore(v.id)}
											>
												<Icon
													name="ArrowCounterClockwise"
													className="h-3.5 w-3.5"
												/>
											</Button>
											<Button
												variant="ghost"
												size="icon"
												className="h-7 w-7 text-destructive"
												title={t("version_delete")}
												onClick={() => handleDelete(v.id)}
											>
												<Icon name="Trash" className="h-3.5 w-3.5" />
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
	);
}
