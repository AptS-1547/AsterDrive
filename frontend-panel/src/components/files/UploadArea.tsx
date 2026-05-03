import type { ReactNode } from "react";
import { forwardRef, useImperativeHandle, useRef } from "react";
import { useTranslation } from "react-i18next";
import { UploadPanel } from "@/components/files/UploadPanel";
import { useUploadAreaManager } from "@/components/files/useUploadAreaManager";
import { Icon } from "@/components/ui/icon";
import { cn } from "@/lib/utils";
import { useAuthStore } from "@/stores/authStore";
import { useFileStore } from "@/stores/fileStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

interface UploadAreaProps {
	children: ReactNode;
}

export interface UploadAreaHandle {
	triggerFileUpload: () => void;
	triggerFolderUpload: () => void;
}

export const UploadArea = forwardRef<UploadAreaHandle, UploadAreaProps>(
	function UploadArea({ children }, ref) {
		const { t } = useTranslation(["core", "files"]);
		const refresh = useFileStore((state) => state.refresh);
		const currentFolderId = useFileStore((state) => state.currentFolderId);
		const breadcrumb = useFileStore((state) => state.breadcrumb);
		const workspace = useWorkspaceStore((state) => state.workspace);
		const refreshUser = useAuthStore((state) => state.refreshUser);
		const fileInputRef = useRef<HTMLInputElement | null>(null);
		const folderInputRef = useRef<HTMLInputElement | null>(null);
		const resumeFileInputRef = useRef<HTMLInputElement | null>(null);
		const {
			activeCount,
			clearCompletedTasks,
			failedCount,
			handleDragEnter,
			handleDragLeave,
			handleDragOver,
			handleDrop,
			handleFileInputChange,
			handleFolderInputChange,
			handleResumeFileChange,
			isDragging,
			overallProgress,
			retryFailedTasks,
			setUploadPanelOpen,
			setUploadAutoClearCompleted,
			setUploadConcurrency,
			successCount,
			totalCount,
			uploadAutoClearCompleted,
			uploadConcurrency,
			uploadPanelOpen,
			uploadTasks,
		} = useUploadAreaManager({
			breadcrumb,
			currentFolderId,
			refresh,
			refreshUser,
			resumeFileInputRef,
			workspace,
		});

		useImperativeHandle(
			ref,
			() => ({
				triggerFileUpload: () => fileInputRef.current?.click(),
				triggerFolderUpload: () => folderInputRef.current?.click(),
			}),
			[],
		);

		const uploadSummary =
			totalCount > 0 && successCount === totalCount
				? t("files:upload_summary_done", { total: totalCount })
				: t("files:upload_summary", {
						total: totalCount,
						success: successCount,
						failed: failedCount,
						active: activeCount,
					});

		return (
			<>
				<input
					ref={fileInputRef}
					type="file"
					data-testid="upload-file-input"
					multiple
					className="hidden"
					onChange={handleFileInputChange}
				/>
				<input
					ref={folderInputRef}
					type="file"
					data-testid="upload-folder-input"
					multiple
					className="hidden"
					// @ts-expect-error webkitdirectory is browser-specific
					webkitdirectory=""
					onChange={handleFolderInputChange}
				/>
				<input
					ref={resumeFileInputRef}
					type="file"
					data-testid="resume-input"
					className="hidden"
					onChange={handleResumeFileChange}
				/>
				{/* biome-ignore lint/a11y/noStaticElementInteractions: drop zone */}
				<div
					className="relative flex min-h-0 flex-1 flex-col overflow-hidden"
					onDragEnter={handleDragEnter}
					onDragLeave={handleDragLeave}
					onDragOver={handleDragOver}
					onDrop={(event) => {
						void handleDrop(event);
					}}
				>
					{children}

					{uploadTasks.length > 0 && (
						<UploadPanel
							open={uploadPanelOpen}
							onToggle={() => setUploadPanelOpen((prev) => !prev)}
							title={t("files:upload")}
							summary={uploadSummary}
							tasks={uploadTasks}
							emptyText={t("no_data")}
							totalCount={totalCount}
							successCount={successCount}
							failedCount={failedCount}
							activeCount={activeCount}
							overallProgress={overallProgress}
							concurrency={uploadConcurrency}
							autoClearCompleted={uploadAutoClearCompleted}
							onConcurrencyChange={setUploadConcurrency}
							onAutoClearCompletedChange={setUploadAutoClearCompleted}
							onRetryFailed={retryFailedTasks}
							retryFailedLabel={t("files:upload_retry")}
							onClearCompleted={clearCompletedTasks}
							clearCompletedLabel={t("files:upload_clear_completed")}
						/>
					)}

					{isDragging && (
						<div
							className={cn(
								"absolute inset-0 z-50 flex flex-col items-center justify-center rounded-lg border-2 border-dashed border-primary bg-background/80 backdrop-blur-sm",
							)}
						>
							<Icon name="Upload" className="mb-3 h-10 w-10 text-primary" />
							<p className="text-lg font-medium text-primary">
								{t("files:drop_files_or_folders")}
							</p>
							<p className="mt-1 text-sm text-muted-foreground">
								{t("files:drop_files_or_folders_desc")}
							</p>
						</div>
					)}
				</div>
			</>
		);
	},
);
