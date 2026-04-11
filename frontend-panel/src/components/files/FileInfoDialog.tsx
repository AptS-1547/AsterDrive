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

function hasFileDetails(file: FileInfo | FileListItem): file is FileInfo {
	return "blob_id" in file && "created_at" in file;
}

function hasFolderDetails(
	folder: FolderInfo | FolderListItem,
): folder is FolderInfo {
	return "created_at" in folder;
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
	const [resolvedFile, setResolvedFile] = useState<FileInfo | null>(null);
	const [fileDetailsLoading, setFileDetailsLoading] = useState(false);
	const [resolvedFolder, setResolvedFolder] = useState<FolderInfo | null>(null);
	const [folderDetailsLoading, setFolderDetailsLoading] = useState(false);
	const [childCount, setChildCount] = useState<{
		folders: number;
		files: number;
	} | null>(null);

	useEffect(() => {
		if (!open || !file) {
			setResolvedFile(null);
			setFileDetailsLoading(false);
			return;
		}
		if (hasFileDetails(file)) {
			setResolvedFile(file);
			setFileDetailsLoading(false);
			return;
		}

		let cancelled = false;
		setResolvedFile(null);
		setFileDetailsLoading(true);
		fileService
			.getFile(file.id)
			.then((data) => {
				if (!cancelled) {
					setResolvedFile(data);
				}
			})
			.catch(() => {
				if (!cancelled) {
					setResolvedFile(null);
				}
			})
			.finally(() => {
				if (!cancelled) {
					setFileDetailsLoading(false);
				}
			});

		return () => {
			cancelled = true;
		};
	}, [file, open]);

	useEffect(() => {
		if (!open || !folder) {
			setResolvedFolder(null);
			setFolderDetailsLoading(false);
			return;
		}
		if (hasFolderDetails(folder)) {
			setResolvedFolder(folder);
			setFolderDetailsLoading(false);
			return;
		}

		let cancelled = false;
		setResolvedFolder(null);
		setFolderDetailsLoading(true);
		fileService
			.getFolderInfo(folder.id)
			.then((data) => {
				if (!cancelled) {
					setResolvedFolder(data);
				}
			})
			.catch(() => {
				if (!cancelled) {
					setResolvedFolder(null);
				}
			})
			.finally(() => {
				if (!cancelled) {
					setFolderDetailsLoading(false);
				}
			});

		return () => {
			cancelled = true;
		};
	}, [folder, open]);

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

	const activeFile = file ? (hasFileDetails(file) ? file : resolvedFile) : null;
	const activeFolder = folder
		? hasFolderDetails(folder)
			? folder
			: resolvedFolder
		: null;
	const loadingText = t("info_loading");

	const rows: InfoRow[] = file
		? [
				{ label: t("info_name"), value: (activeFile ?? file).name },
				{
					label: t("info_size"),
					value: formatBytes((activeFile ?? file).size),
				},
				{ label: t("info_mime"), value: (activeFile ?? file).mime_type },
				{
					label: t("info_created"),
					value: activeFile?.created_at
						? formatDateAbsolute(activeFile.created_at)
						: fileDetailsLoading
							? loadingText
							: "—",
				},
				{
					label: t("info_modified"),
					value: formatDateAbsolute((activeFile ?? file).updated_at),
				},
				{
					label: t("info_locked"),
					value: (activeFile ?? file).is_locked
						? t("info_locked_yes")
						: t("info_locked_no"),
				},
				{
					label: t("info_blob_id"),
					value:
						activeFile != null
							? String(activeFile.blob_id)
							: fileDetailsLoading
								? loadingText
								: "—",
				},
			]
		: folder
			? [
					{ label: t("info_name"), value: (activeFolder ?? folder).name },
					{
						label: t("info_created"),
						value: activeFolder?.created_at
							? formatDateAbsolute(activeFolder.created_at)
							: folderDetailsLoading
								? loadingText
								: "—",
					},
					{
						label: t("info_modified"),
						value: formatDateAbsolute((activeFolder ?? folder).updated_at),
					},
					{
						label: t("info_locked"),
						value: (activeFolder ?? folder).is_locked
							? t("info_locked_yes")
							: t("info_locked_no"),
					},
					{
						label: t("info_policy_id"),
						value:
							activeFolder == null
								? folderDetailsLoading
									? loadingText
									: "—"
								: activeFolder.policy_id != null
									? String(activeFolder.policy_id)
									: "—",
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

	const title = file
		? (activeFile ?? file).name
		: ((activeFolder ?? folder)?.name ?? "");

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
