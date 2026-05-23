import { Fragment, type ReactNode, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import { UserAvatarImage } from "@/components/common/UserAvatarImage";
import { ViewToggle } from "@/components/common/ViewToggle";
import { ReadOnlyFileCollection } from "@/components/files/ReadOnlyFileCollection";
import {
	Breadcrumb,
	BreadcrumbItem,
	BreadcrumbLink,
	BreadcrumbList,
	BreadcrumbPage,
	BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Icon } from "@/components/ui/icon";
import { PAGE_SECTION_PADDING_CLASS } from "@/lib/constants";
import type {
	FileInfo,
	FileListItem,
	FolderContents,
	SharePublicInfo,
} from "@/types/api";
import { ShareFolderContentSkeleton } from "./ShareFolderSkeleton";
import { ShareMetaLine, SharePageShell } from "./ShareViewShell";
import {
	getAccessSummary,
	getDownloadSummary,
	getExpirySummary,
} from "./shareViewSummaries";
import type { ShareBreadcrumbItem } from "./types";

interface ShareFolderViewProps {
	breadcrumb: ShareBreadcrumbItem[];
	folderContents: FolderContents | null;
	hasMoreFiles: boolean;
	info: SharePublicInfo;
	loadingMore: boolean;
	navigating: boolean;
	previewElement: ReactNode;
	sentinelRef: RefObject<HTMLDivElement | null>;
	shareOwnerText: string;
	token: string;
	viewMode: "grid" | "list";
	onFileDownload: (file: FileListItem) => void;
	onFilePreview: (file: FileInfo | FileListItem) => void;
	onNavigateToFolder: (folderId: number | null, folderName?: string) => void;
	onViewModeChange: (viewMode: "grid" | "list") => void;
}

function ShareFolderBreadcrumb({
	breadcrumb,
	onNavigateToFolder,
}: {
	breadcrumb: ShareBreadcrumbItem[];
	onNavigateToFolder: (folderId: number | null, folderName?: string) => void;
}) {
	if (breadcrumb.length <= 1) return null;

	return (
		<Breadcrumb>
			<BreadcrumbList className="gap-2">
				{breadcrumb.map((item, i) => (
					<Fragment key={item.id ?? "root"}>
						{i > 0 && (
							<BreadcrumbSeparator className="mx-0.5 text-muted-foreground/45" />
						)}
						<BreadcrumbItem>
							{i < breadcrumb.length - 1 ? (
								<BreadcrumbLink
									className="cursor-pointer rounded-md px-1.5 py-0.5 text-muted-foreground"
									onClick={() => onNavigateToFolder(item.id, item.name)}
								>
									{item.name}
								</BreadcrumbLink>
							) : (
								<BreadcrumbPage className="text-base font-semibold text-foreground">
									{item.name}
								</BreadcrumbPage>
							)}
						</BreadcrumbItem>
					</Fragment>
				))}
			</BreadcrumbList>
		</Breadcrumb>
	);
}

export function ShareFolderView({
	breadcrumb,
	folderContents,
	hasMoreFiles,
	info,
	loadingMore,
	navigating,
	onFileDownload,
	onFilePreview,
	onNavigateToFolder,
	onViewModeChange,
	previewElement,
	sentinelRef,
	shareOwnerText,
	token,
	viewMode,
}: ShareFolderViewProps) {
	const { t } = useTranslation(["core", "share", "files", "errors"]);
	const breadcrumbElement = (
		<ShareFolderBreadcrumb
			breadcrumb={breadcrumb}
			onNavigateToFolder={onNavigateToFolder}
		/>
	);

	return (
		<SharePageShell>
			<main className="flex min-h-0 flex-1 flex-col overflow-hidden">
				<div
					className={`border-b border-border/65 bg-card/55 ${PAGE_SECTION_PADDING_CLASS}`}
				>
					<div className="mx-auto flex w-full max-w-7xl flex-col gap-3 py-3 sm:flex-row sm:items-center sm:justify-between">
						<div className="flex min-w-0 items-center gap-3">
							<div className="flex size-12 shrink-0 items-center justify-center rounded-lg bg-amber-500/12 text-amber-600 dark:text-amber-300">
								<Icon name="Folder" className="size-6" />
							</div>
							<div className="min-w-0">
								<h1 className="truncate text-lg font-semibold leading-tight sm:text-xl">
									{info.name}
								</h1>
								<div className="mt-1 flex min-w-0 items-center gap-2">
									<UserAvatarImage
										avatar={info.shared_by.avatar}
										name={info.shared_by.name}
										size="sm"
										className="size-5 rounded-md text-[10px]"
									/>
									<ShareMetaLine
										items={[
											shareOwnerText,
											getDownloadSummary(info, t),
											getExpirySummary(info, t),
											getAccessSummary(info, t),
										]}
										className="min-w-0 text-xs"
									/>
								</div>
							</div>
						</div>
						<ViewToggle value={viewMode} onChange={onViewModeChange} />
					</div>
				</div>
				<div className={`min-h-0 flex-1 py-3 ${PAGE_SECTION_PADDING_CLASS}`}>
					<section className="mx-auto flex h-full w-full max-w-7xl flex-col overflow-hidden rounded-lg border border-border/70 bg-card/70 shadow-xs dark:bg-card/40 dark:shadow-none">
						{breadcrumb.length > 1 ? (
							<div className="flex flex-col gap-3 border-b border-border/65 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
								<div className="flex min-w-0 items-center gap-2">
									<Icon
										name="FolderOpen"
										className="size-5 shrink-0 text-amber-500"
									/>
									<div className="min-w-0 flex-1">{breadcrumbElement}</div>
								</div>
							</div>
						) : null}
						<div className="min-h-0 flex-1 overflow-auto">
							{navigating ? (
								<ShareFolderContentSkeleton viewMode={viewMode} />
							) : folderContents ? (
								<>
									<ReadOnlyFileCollection
										folders={folderContents.folders}
										files={folderContents.files}
										viewMode={viewMode}
										onFileClick={onFilePreview}
										onFileDownload={onFileDownload}
										onFolderClick={(folder) =>
											onNavigateToFolder(folder.id, folder.name)
										}
										getThumbnailPath={(file) =>
											`/s/${token}/files/${file.id}/thumbnail`
										}
										emptyTitle={t("empty_folder")}
										emptyDescription={t("files:folder_empty_desc")}
									/>
									{hasMoreFiles && (
										<div ref={sentinelRef} className="flex justify-center py-4">
											{loadingMore && (
												<div className="size-5 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-muted-foreground" />
											)}
										</div>
									)}
								</>
							) : (
								<div className="p-6 text-sm text-muted-foreground">
									{t("loading_contents")}
								</div>
							)}
						</div>
					</section>
				</div>
			</main>
			{previewElement}
		</SharePageShell>
	);
}
