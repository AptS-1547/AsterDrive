import { PreviewAppIcon } from "@/components/common/PreviewAppIcon";
import { FileTypeIcon } from "@/components/files/FileTypeIcon";
import { Button } from "@/components/ui/button";
import { DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { formatBytes } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { FileInfo, FileListItem } from "@/types/api";
import { AnimatedCollapsible } from "./AnimatedCollapsible";
import type { OpenWithMode, OpenWithOption } from "./types";

interface FilePreviewMethodChooserProps {
	file: FileInfo | FileListItem;
	activeMode: OpenWithMode | null;
	allOptions: OpenWithOption[];
	visibleOptions: OpenWithOption[];
	hiddenOptions: OpenWithOption[];
	showAllOpenMethods: boolean;
	getOptionLabel: (option: OpenWithOption) => string;
	onClose: () => void;
	onSelect: (mode: OpenWithMode) => void;
	onShowAllOpenMethods: () => void;
	chooseOpenMethodLabel: string;
	closeLabel: string;
	moreOpenMethodsLabel: string;
}

export function FilePreviewMethodChooser({
	file,
	activeMode,
	allOptions,
	visibleOptions,
	hiddenOptions,
	showAllOpenMethods,
	getOptionLabel,
	onClose,
	onSelect,
	onShowAllOpenMethods,
	chooseOpenMethodLabel,
	closeLabel,
	moreOpenMethodsLabel,
}: FilePreviewMethodChooserProps) {
	return (
		<>
			<DialogHeader className="border-b px-5 py-4">
				<div className="flex items-center gap-3">
					<div className="flex h-10 w-10 items-center justify-center rounded-xl bg-muted text-muted-foreground">
						<FileTypeIcon
							mimeType={file.mime_type}
							fileName={file.name}
							className="h-5 w-5"
						/>
					</div>
					<div className="min-w-0 flex-1">
						<DialogTitle className="truncate">
							{chooseOpenMethodLabel}
						</DialogTitle>
						<p className="mt-1 truncate text-sm text-muted-foreground">
							{file.name} · {formatBytes(file.size)}
						</p>
					</div>
					<Button
						variant="ghost"
						size="icon-sm"
						onClick={onClose}
						aria-label={closeLabel}
						title={closeLabel}
					>
						<Icon name="X" className="h-4 w-4" />
						<span className="sr-only">{closeLabel}</span>
					</Button>
				</div>
			</DialogHeader>
			<div className="min-h-0 overflow-y-auto p-4">
				<div className="grid gap-2">
					{visibleOptions.map((option) => {
						const isActive = option.key === activeMode;
						return (
							<OpenMethodButton
								key={option.key}
								option={option}
								isActive={isActive}
								label={getOptionLabel(option)}
								onSelect={onSelect}
							/>
						);
					})}
					<AnimatedCollapsible open={showAllOpenMethods}>
						<div className="grid gap-2">
							{hiddenOptions.map((option) => {
								const isActive = option.key === activeMode;
								return (
									<OpenMethodButton
										key={option.key}
										option={option}
										isActive={isActive}
										label={getOptionLabel(option)}
										onSelect={onSelect}
									/>
								);
							})}
						</div>
					</AnimatedCollapsible>
					{!showAllOpenMethods && allOptions.length > 0 ? (
						<Button
							variant="ghost"
							className="h-auto justify-start rounded-xl border border-dashed px-3.5 py-2.5 text-left text-muted-foreground"
							onClick={onShowAllOpenMethods}
						>
							<div className="flex w-full items-center gap-2.5">
								<div className="min-w-0 flex-1">
									<div className="font-medium">{moreOpenMethodsLabel}</div>
								</div>
								<Icon name="CaretDown" className="h-4 w-4" />
							</div>
						</Button>
					) : null}
				</div>
			</div>
		</>
	);
}

function OpenMethodButton({
	option,
	isActive,
	label,
	onSelect,
}: {
	option: OpenWithOption;
	isActive: boolean;
	label: string;
	onSelect: (mode: OpenWithMode) => void;
}) {
	return (
		<Button
			variant="ghost"
			className={cn(
				"h-auto justify-start rounded-xl border px-3.5 py-2.5 text-left",
				isActive && "border-primary bg-accent text-foreground",
			)}
			onClick={() => onSelect(option.key)}
		>
			<div className="flex w-full items-center gap-2.5">
				<div className="flex h-9 w-9 items-center justify-center rounded-lg bg-muted text-muted-foreground">
					<PreviewAppIcon icon={option.icon} className="h-4 w-4" />
				</div>
				<div className="min-w-0 flex-1">
					<div className="truncate font-medium">{label}</div>
				</div>
				<Icon
					name={isActive ? "Check" : "CaretRight"}
					className="h-4 w-4 text-muted-foreground"
				/>
			</div>
		</Button>
	);
}
