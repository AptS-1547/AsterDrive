import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileTypeIcon } from "@/components/files/FileTypeIcon";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatBytes } from "@/lib/format";
import { fileService } from "@/services/fileService";
import type { FileInfo } from "@/types/api";
import { BlobMediaPreview } from "./BlobMediaPreview";
import { CsvTablePreview } from "./CsvTablePreview";
import { detectFilePreviewProfile } from "./file-capabilities";
import { JsonPreview } from "./JsonPreview";
import { MarkdownPreview } from "./MarkdownPreview";
import {
	getStoredOpenWithPreference,
	setStoredOpenWithPreference,
} from "./open-with-preferences";
import { PreviewModeSwitch } from "./PreviewModeSwitch";
import { PreviewUnavailable } from "./PreviewUnavailable";
import { TextCodePreview } from "./TextCodePreview";
import type { OpenWithMode } from "./types";
import { UnsavedChangesGuard } from "./UnsavedChangesGuard";
import { XmlPreview } from "./XmlPreview";

interface FilePreviewDialogProps {
	file: FileInfo;
	onClose: () => void;
	onFileUpdated?: () => void;
	downloadPath?: string;
	editable?: boolean;
}

export function FilePreviewDialog({
	file,
	onClose,
	onFileUpdated,
	downloadPath,
	editable = true,
}: FilePreviewDialogProps) {
	const { t } = useTranslation(["files", "common"]);
	const profile = useMemo(() => detectFilePreviewProfile(file), [file]);
	const preferredMode = useMemo(() => {
		const stored = getStoredOpenWithPreference(profile.category);
		if (stored && profile.options.some((option) => option.mode === stored)) {
			return stored;
		}
		return profile.defaultMode;
	}, [profile.category, profile.defaultMode, profile.options]);
	const [mode, setMode] = useState<OpenWithMode | null>(preferredMode);
	const [isDirty, setIsDirty] = useState(false);
	const [confirmOpen, setConfirmOpen] = useState(false);
	const [pendingMode, setPendingMode] = useState<OpenWithMode | null>(null);
	const activeMode = mode ?? preferredMode;
	const resolvedDownloadPath =
		downloadPath ?? fileService.downloadPath(file.id);

	const closeWithGuard = useCallback(() => {
		if (isDirty) {
			setPendingMode(null);
			setConfirmOpen(true);
			return;
		}
		onClose();
	}, [isDirty, onClose]);

	const handleModeChange = useCallback(
		(nextMode: OpenWithMode) => {
			if (nextMode === activeMode) return;
			if (isDirty) {
				setPendingMode(nextMode);
				setConfirmOpen(true);
				return;
			}
			setMode(nextMode);
			setStoredOpenWithPreference(profile.category, nextMode);
		},
		[activeMode, isDirty, profile.category],
	);

	const handleDiscardChanges = useCallback(() => {
		setConfirmOpen(false);
		setIsDirty(false);
		if (pendingMode) {
			setMode(pendingMode);
			setStoredOpenWithPreference(profile.category, pendingMode);
			setPendingMode(null);
			return;
		}
		onClose();
	}, [onClose, pendingMode, profile.category]);

	const body = (() => {
		if (!activeMode) return <PreviewUnavailable />;
		if (
			activeMode === "image" ||
			activeMode === "video" ||
			activeMode === "audio" ||
			activeMode === "pdf"
		) {
			return (
				<BlobMediaPreview
					file={file}
					mode={activeMode}
					path={resolvedDownloadPath}
				/>
			);
		}
		if (activeMode === "markdown") {
			return <MarkdownPreview path={resolvedDownloadPath} />;
		}
		if (activeMode === "table") {
			return (
				<CsvTablePreview
					path={resolvedDownloadPath}
					delimiter={profile.category === "tsv" ? "\t" : ","}
				/>
			);
		}
		if (activeMode === "formatted" && profile.category === "json") {
			return <JsonPreview path={resolvedDownloadPath} />;
		}
		if (activeMode === "formatted" && profile.category === "xml") {
			return <XmlPreview path={resolvedDownloadPath} mode="formatted" />;
		}
		if (activeMode === "code") {
			return (
				<TextCodePreview
					file={file}
					path={resolvedDownloadPath}
					onFileUpdated={onFileUpdated}
					onDirtyChange={setIsDirty}
					editable={editable}
				/>
			);
		}
		return <PreviewUnavailable />;
	})();

	return (
		<>
			<Dialog open onOpenChange={(open) => !open && closeWithGuard()}>
				<DialogContent
					showCloseButton={false}
					className="flex h-[90vh] w-[min(96vw,1200px)] max-w-[min(96vw,1200px)] flex-col gap-0 overflow-hidden p-0 sm:max-w-[min(96vw,1200px)]"
				>
					<DialogHeader className="gap-0 border-b px-4 py-3">
						<div className="flex items-center gap-3">
							<div className="flex h-9 w-9 items-center justify-center rounded-lg bg-muted text-muted-foreground">
								<FileTypeIcon
									mimeType={file.mime_type}
									fileName={file.name}
									className="h-5 w-5"
								/>
							</div>
							<div className="min-w-0 flex-1">
								<DialogTitle className="truncate text-sm font-semibold">
									{file.name}
								</DialogTitle>
								<div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
									<span>{file.mime_type}</span>
									<span>·</span>
									<span>{formatBytes(file.size)}</span>
									<span>·</span>
									<span>{profile.category}</span>
								</div>
							</div>
							<Button variant="ghost" size="icon-sm" onClick={closeWithGuard}>
								<Icon name="X" className="h-4 w-4" />
							</Button>
						</div>
					</DialogHeader>
					<div className="flex items-center gap-3 border-b bg-muted/20 px-4 py-2">
						<PreviewModeSwitch
							options={profile.options}
							value={activeMode ?? "code"}
							onChange={handleModeChange}
						/>
						<div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
							<Icon name="Eye" className="h-3.5 w-3.5" />
							<span>
								{t("files:open_modes_count", { count: profile.options.length })}
							</span>
						</div>
					</div>
					<ScrollArea className="min-h-0 flex-1 bg-muted/20">
						<div className="h-full min-h-full w-full p-3">{body}</div>
					</ScrollArea>
				</DialogContent>
			</Dialog>
			<UnsavedChangesGuard
				open={confirmOpen}
				onOpenChange={setConfirmOpen}
				onConfirm={handleDiscardChanges}
			/>
		</>
	);
}
