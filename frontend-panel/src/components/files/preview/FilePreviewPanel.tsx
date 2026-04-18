import type { ReactNode } from "react";
import { FileTypeIcon } from "@/components/files/FileTypeIcon";
import { Button } from "@/components/ui/button";
import { DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatBytes } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { FileInfo, FileListItem } from "@/types/api";

interface FilePreviewPanelProps {
	file: FileInfo | FileListItem;
	body: ReactNode;
	allOptionsCount: number;
	usesInnerScroll: boolean;
	fillsViewportHeight: boolean;
	isExpanded: boolean;
	isDirty: boolean;
	onChooseOpenMethod: () => void;
	onToggleExpand: () => void;
	onClose: () => void;
	chooseOpenMethodLabel: string;
	enterFullscreenLabel: string;
	exitFullscreenLabel: string;
	closeLabel: string;
}

export function FilePreviewPanel({
	file,
	body,
	allOptionsCount,
	usesInnerScroll,
	fillsViewportHeight,
	isExpanded,
	isDirty,
	onChooseOpenMethod,
	onToggleExpand,
	onClose,
	chooseOpenMethodLabel,
	enterFullscreenLabel,
	exitFullscreenLabel,
	closeLabel,
}: FilePreviewPanelProps) {
	const fullscreenLabel = isExpanded
		? exitFullscreenLabel
		: enterFullscreenLabel;

	return (
		<>
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
						<DialogTitle className="flex items-center gap-2 text-sm font-semibold">
							<span className="min-w-0 truncate">{file.name}</span>
							<span className="shrink-0 text-xs font-normal text-muted-foreground">
								· {formatBytes(file.size)}
							</span>
						</DialogTitle>
					</div>
					<div className="flex items-center gap-1">
						{allOptionsCount > 1 ? (
							<Button
								variant="ghost"
								size="sm"
								onClick={onChooseOpenMethod}
								disabled={isDirty}
								aria-label={chooseOpenMethodLabel}
								title={chooseOpenMethodLabel}
							>
								{chooseOpenMethodLabel}
							</Button>
						) : null}
						<Button
							variant="ghost"
							size="icon-sm"
							onClick={onToggleExpand}
							aria-label={fullscreenLabel}
							title={fullscreenLabel}
						>
							<Icon
								name={isExpanded ? "ArrowsInCardinal" : "ArrowsOutCardinal"}
								className="h-4 w-4"
							/>
							<span className="sr-only">{fullscreenLabel}</span>
						</Button>
						<Button
							variant="ghost"
							size="icon-sm"
							onClick={onClose}
							aria-label={closeLabel}
							title={closeLabel}
						>
							<Icon name="X" className="h-4 w-4" />
						</Button>
					</div>
				</div>
			</DialogHeader>
			{usesInnerScroll ? (
				<div
					className={cn(
						"bg-muted/20 p-3",
						(fillsViewportHeight || isExpanded) && "min-h-0 flex-1",
					)}
				>
					{body}
				</div>
			) : (
				<ScrollArea
					className={cn(
						"bg-muted/20",
						(fillsViewportHeight || isExpanded) && "min-h-0 flex-1",
					)}
				>
					<div
						className={cn(
							"w-full p-3",
							(fillsViewportHeight || isExpanded) && "h-full min-h-full",
						)}
					>
						{body}
					</div>
				</ScrollArea>
			)}
		</>
	);
}
