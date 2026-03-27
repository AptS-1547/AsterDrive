import { Fragment, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
	Dialog,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { formatBytes, formatDateAbsolute } from "@/lib/format";
import { fileService } from "@/services/fileService";
import type {
	FileInfo,
	FileListItem,
	FolderInfo,
	FolderListItem,
} from "@/types/api";

interface FileInfoDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	file?: FileInfo | FileListItem;
	folder?: FolderInfo | FolderListItem;
}

interface InfoRow {
	label: string;
	value: string;
}

function InfoTable({ rows }: { rows: InfoRow[] }) {
	return (
		<dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
			{rows.map((row) => (
				<Fragment key={row.label}>
					<dt className="text-muted-foreground whitespace-nowrap">
						{row.label}
					</dt>
					<dd className="font-mono break-all">{row.value}</dd>
				</Fragment>
			))}
		</dl>
	);
}

export function FileInfoDialog({
	open,
	onOpenChange,
	file,
	folder,
}: FileInfoDialogProps) {
	const { t } = useTranslation("files");
	const [childCount, setChildCount] = useState<{
		folders: number;
		files: number;
	} | null>(null);

	useEffect(() => {
		if (!open || !folder) {
			setChildCount(null);
			return;
		}
		fileService
			.listFolder(folder.id, { folder_limit: 0, file_limit: 0 })
			.then((res) => {
				setChildCount({
					folders: res.folders_total,
					files: res.files_total,
				});
			})
			.catch(() => setChildCount(null));
	}, [open, folder]);

	const rows: InfoRow[] = file
		? [
				{ label: t("info_name"), value: file.name },
				{ label: t("info_size"), value: formatBytes(file.size) },
				{ label: t("info_mime"), value: file.mime_type },
				{
					label: t("info_created"),
					value: formatDateAbsolute(file.created_at),
				},
				{
					label: t("info_modified"),
					value: formatDateAbsolute(file.updated_at),
				},
				{
					label: t("info_locked"),
					value: file.is_locked ? t("info_locked_yes") : t("info_locked_no"),
				},
				{ label: t("info_blob_id"), value: String(file.blob_id) },
			]
		: folder
			? [
					{ label: t("info_name"), value: folder.name },
					{
						label: t("info_created"),
						value: formatDateAbsolute(folder.created_at),
					},
					{
						label: t("info_modified"),
						value: formatDateAbsolute(folder.updated_at),
					},
					{
						label: t("info_locked"),
						value: folder.is_locked
							? t("info_locked_yes")
							: t("info_locked_no"),
					},
					{
						label: t("info_policy_id"),
						value: folder.policy_id != null ? String(folder.policy_id) : "—",
					},
					{
						label: t("info_children"),
						value:
							childCount != null
								? t("info_children_count", {
										folders: childCount.folders,
										files: childCount.files,
									})
								: t("info_loading"),
					},
				]
			: [];

	const title = file ? file.name : (folder?.name ?? "");

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent>
				<DialogHeader>
					<DialogTitle>{title}</DialogTitle>
				</DialogHeader>
				<InfoTable rows={rows} />
				<DialogFooter />
			</DialogContent>
		</Dialog>
	);
}
