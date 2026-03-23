import type { DragEvent, ReactNode } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
	UploadPanel,
	type UploadTaskView,
} from "@/components/files/UploadPanel";
import { Icon } from "@/components/ui/icon";
import { cn } from "@/lib/utils";
import { api } from "@/services/http";
import {
	type InitUploadResponse,
	uploadService,
} from "@/services/uploadService";
import { useFileStore } from "@/stores/fileStore";

interface UploadAreaProps {
	children: ReactNode;
}

type UploadMode = "direct" | "chunked" | "presigned";
type UploadStatus =
	| "queued"
	| "initializing"
	| "uploading"
	| "processing"
	| "completed"
	| "failed"
	| "cancelled";

interface UploadTask {
	id: string;
	file: File;
	mode: UploadMode | null;
	status: UploadStatus;
	progress: number;
	error: string | null;
	uploadId: string | null;
	completedChunks?: number;
	totalChunks?: number;
}

const MAX_FILE_CONCURRENT = 2;
const CHUNK_CONCURRENT = 3;
const CHUNK_MAX_RETRIES = 3;

function createTaskId() {
	return `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export function UploadArea({ children }: UploadAreaProps) {
	const { t } = useTranslation(["files", "common"]);
	const refresh = useFileStore((s) => s.refresh);
	const currentFolderId = useFileStore((s) => s.currentFolderId);
	const currentFolderIdRef = useRef(currentFolderId);
	const [isDragging, setIsDragging] = useState(false);
	const dragCounter = useRef(0);
	const [uploadPanelOpen, setUploadPanelOpen] = useState(true);
	const [tasks, setTasks] = useState<UploadTask[]>([]);
	const tasksRef = useRef<UploadTask[]>([]);
	const abortFlagsRef = useRef(new Map<string, boolean>());
	const directAbortRef = useRef(new Map<string, AbortController>());
	const presignedXhrRef = useRef(new Map<string, XMLHttpRequest>());
	const refreshTimeoutRef = useRef<number | null>(null);

	useEffect(() => {
		currentFolderIdRef.current = currentFolderId;
	}, [currentFolderId]);

	useEffect(() => {
		tasksRef.current = tasks;
	}, [tasks]);

	useEffect(() => {
		return () => {
			for (const controller of directAbortRef.current.values()) {
				controller.abort();
			}
			for (const xhr of presignedXhrRef.current.values()) {
				xhr.abort();
			}
			if (refreshTimeoutRef.current !== null) {
				window.clearTimeout(refreshTimeoutRef.current);
			}
		};
	}, []);

	const scheduleRefresh = useCallback(() => {
		if (refreshTimeoutRef.current !== null) return;
		refreshTimeoutRef.current = window.setTimeout(() => {
			refreshTimeoutRef.current = null;
			void refresh();
		}, 300);
	}, [refresh]);

	const patchTask = useCallback(
		(taskId: string, patch: Partial<UploadTask>) => {
			setTasks((prev) =>
				prev.map((task) => (task.id === taskId ? { ...task, ...patch } : task)),
			);
		},
		[],
	);

	const clearCompletedTasks = useCallback(() => {
		setTasks((prev) => prev.filter((task) => task.status !== "completed"));
	}, []);

	const markTaskFailed = useCallback(
		(taskId: string, message: string) => {
			patchTask(taskId, {
				status: "failed",
				error: message,
			});
		},
		[patchTask],
	);

	const runDirectUpload = useCallback(
		async (task: UploadTask) => {
			patchTask(task.id, { mode: "direct", status: "uploading", progress: 0 });
			const controller = new AbortController();
			directAbortRef.current.set(task.id, controller);

			try {
				const formData = new FormData();
				formData.append("file", task.file);
				const folderId = currentFolderIdRef.current;
				const path =
					folderId !== null
						? `/files/upload?folder_id=${folderId}`
						: "/files/upload";

				await api.client.post(path, formData, {
					headers: { "Content-Type": "multipart/form-data" },
					signal: controller.signal,
					onUploadProgress: (event) => {
						if (!event.total) return;
						patchTask(task.id, {
							progress: Math.round((event.loaded / event.total) * 100),
						});
					},
				});

				patchTask(task.id, {
					status: "completed",
					progress: 100,
					error: null,
				});
				scheduleRefresh();
			} catch (error) {
				if (controller.signal.aborted) {
					patchTask(task.id, { status: "cancelled", error: null });
					return;
				}
				const message =
					error instanceof Error ? error.message : t("common:unexpected_error");
				markTaskFailed(task.id, message);
			} finally {
				directAbortRef.current.delete(task.id);
			}
		},
		[markTaskFailed, patchTask, scheduleRefresh, t],
	);

	const runChunkedUpload = useCallback(
		async (task: UploadTask, init: InitUploadResponse) => {
			const uploadId = init.upload_id as string;
			const chunkSize = init.chunk_size as number;
			const totalChunks = init.total_chunks as number;
			abortFlagsRef.current.set(task.id, false);
			patchTask(task.id, {
				mode: "chunked",
				status: "uploading",
				uploadId,
				totalChunks,
				completedChunks: 0,
				progress: 0,
			});

			let completed = 0;
			const queue = Array.from({ length: totalChunks }, (_, index) => index);

			const uploadOneChunk = async () => {
				while (queue.length > 0) {
					if (abortFlagsRef.current.get(task.id)) return;
					const chunkNumber = queue.shift();
					if (chunkNumber === undefined) return;
					const start = chunkNumber * chunkSize;
					const end = Math.min(start + chunkSize, task.file.size);
					const blob = task.file.slice(start, end);

					let lastError: Error | null = null;
					for (let attempt = 0; attempt < CHUNK_MAX_RETRIES; attempt++) {
						try {
							await uploadService.uploadChunk(uploadId, chunkNumber, blob);
							lastError = null;
							break;
						} catch (error) {
							lastError =
								error instanceof Error ? error : new Error(String(error));
							if (attempt < CHUNK_MAX_RETRIES - 1) {
								await new Promise((resolve) =>
									setTimeout(resolve, 1000 * 2 ** attempt),
								);
							}
						}
					}

					if (lastError) throw lastError;
					completed += 1;
					patchTask(task.id, {
						completedChunks: completed,
						progress: Math.round((completed / totalChunks) * 95),
					});
				}
			};

			try {
				const workers = Array.from(
					{ length: Math.min(CHUNK_CONCURRENT, queue.length || 1) },
					() => uploadOneChunk(),
				);
				await Promise.all(workers);

				if (abortFlagsRef.current.get(task.id)) {
					patchTask(task.id, { status: "cancelled", error: null });
					return;
				}

				patchTask(task.id, { status: "processing", progress: 95 });
				await uploadService.completeUpload(uploadId);
				patchTask(task.id, {
					status: "completed",
					progress: 100,
					error: null,
				});
				scheduleRefresh();
			} catch (error) {
				if (abortFlagsRef.current.get(task.id)) {
					patchTask(task.id, { status: "cancelled", error: null });
					return;
				}
				const message =
					error instanceof Error ? error.message : t("common:unexpected_error");
				markTaskFailed(task.id, message);
			} finally {
				abortFlagsRef.current.delete(task.id);
			}
		},
		[markTaskFailed, patchTask, scheduleRefresh, t],
	);

	const runPresignedUpload = useCallback(
		async (task: UploadTask, init: InitUploadResponse) => {
			const uploadId = init.upload_id as string;
			const presignedUrl = init.presigned_url as string;
			patchTask(task.id, {
				mode: "presigned",
				status: "uploading",
				uploadId,
				progress: 0,
			});

			try {
				await uploadService.presignedUpload(
					presignedUrl,
					task.file,
					(loaded, total) => {
						patchTask(task.id, {
							progress: Math.round((loaded / total) * 90),
						});
					},
					(xhr) => {
						presignedXhrRef.current.set(task.id, xhr);
					},
				);

				patchTask(task.id, { status: "processing", progress: 90 });
				await uploadService.completeUpload(uploadId);
				patchTask(task.id, {
					status: "completed",
					progress: 100,
					error: null,
				});
				scheduleRefresh();
			} catch (error) {
				const message =
					error instanceof Error ? error.message : t("common:unexpected_error");
				if (message.includes("abort")) {
					patchTask(task.id, { status: "cancelled", error: null });
					return;
				}
				markTaskFailed(task.id, message);
			} finally {
				presignedXhrRef.current.delete(task.id);
			}
		},
		[markTaskFailed, patchTask, scheduleRefresh, t],
	);

	const runTask = useCallback(
		async (taskId: string) => {
			const task = tasksRef.current.find((item) => item.id === taskId);
			if (!task || task.status !== "queued") return;

			patchTask(taskId, { status: "initializing", error: null, progress: 0 });
			try {
				const init = await uploadService.initUpload({
					filename: task.file.name,
					total_size: task.file.size,
					folder_id: currentFolderIdRef.current,
				});
				if (init.mode === "chunked") {
					await runChunkedUpload(task, init);
				} else if (init.mode === "presigned") {
					await runPresignedUpload(task, init);
				} else {
					await runDirectUpload(task);
				}
			} catch (error) {
				const message =
					error instanceof Error ? error.message : t("common:unexpected_error");
				markTaskFailed(taskId, message);
			}
		},
		[
			markTaskFailed,
			patchTask,
			runChunkedUpload,
			runDirectUpload,
			runPresignedUpload,
			t,
		],
	);

	useEffect(() => {
		const activeCount = tasks.filter((task) =>
			["initializing", "uploading", "processing"].includes(task.status),
		).length;
		if (activeCount >= MAX_FILE_CONCURRENT) return;
		const queued = tasks.filter((task) => task.status === "queued");
		if (queued.length === 0) return;
		const nextTasks = queued.slice(0, MAX_FILE_CONCURRENT - activeCount);
		nextTasks.forEach((task) => {
			void runTask(task.id);
		});
	}, [runTask, tasks]);

	const cancelTask = useCallback(
		async (taskId: string) => {
			const task = tasksRef.current.find((item) => item.id === taskId);
			if (!task) return;

			if (task.mode === "direct") {
				directAbortRef.current.get(taskId)?.abort();
				patchTask(taskId, { status: "cancelled", error: null });
				return;
			}

			if (task.mode === "presigned") {
				presignedXhrRef.current.get(taskId)?.abort();
				if (task.uploadId) {
					try {
						await uploadService.cancelUpload(task.uploadId);
					} catch {
						// ignore
					}
				}
				patchTask(taskId, { status: "cancelled", error: null });
				return;
			}

			abortFlagsRef.current.set(taskId, true);
			if (task.uploadId) {
				try {
					await uploadService.cancelUpload(task.uploadId);
				} catch {
					// ignore
				}
			}
			patchTask(taskId, { status: "cancelled", error: null });
		},
		[patchTask],
	);

	const retryTask = useCallback(
		(taskId: string) => {
			patchTask(taskId, {
				status: "queued",
				progress: 0,
				error: null,
				uploadId: null,
				completedChunks: 0,
				totalChunks: 0,
			});
			setUploadPanelOpen(true);
		},
		[patchTask],
	);

	const addFiles = useCallback((files: FileList | null) => {
		if (!files || files.length === 0) return;
		const nextTasks = Array.from(files).map((file) => ({
			id: createTaskId(),
			file,
			mode: null,
			status: "queued" as UploadStatus,
			progress: 0,
			error: null,
			uploadId: null,
		}));
		setTasks((prev) => [...nextTasks, ...prev]);
		setUploadPanelOpen(true);
	}, []);

	const handleDragEnter = (e: DragEvent<HTMLDivElement>) => {
		e.preventDefault();
		dragCounter.current += 1;
		if (e.dataTransfer.types.includes("Files")) setIsDragging(true);
	};
	const handleDragLeave = (e: DragEvent<HTMLDivElement>) => {
		e.preventDefault();
		dragCounter.current -= 1;
		if (dragCounter.current === 0) setIsDragging(false);
	};
	const handleDragOver = (e: DragEvent<HTMLDivElement>) => e.preventDefault();
	const handleDrop = (e: DragEvent<HTMLDivElement>) => {
		e.preventDefault();
		dragCounter.current = 0;
		setIsDragging(false);
		addFiles(e.dataTransfer.files);
	};

	const uploadTasks: UploadTaskView[] = tasks.map((task) => {
		const modeLabel =
			task.mode === "chunked"
				? "Chunked"
				: task.mode === "presigned"
					? "S3"
					: task.mode === "direct"
						? "Direct"
						: "Pending";

		const statusLabel =
			task.status === "queued"
				? t("files:processing")
				: task.status === "initializing"
					? t("files:processing")
					: task.status === "uploading"
						? t("files:uploading_to_storage")
						: task.status === "processing"
							? t("files:upload_processing")
							: task.status === "completed"
								? t("files:upload_success")
								: task.status === "cancelled"
									? t("files:upload_dismiss")
									: t("files:upload_failed");

		const detail =
			task.status === "failed"
				? (task.error ?? t("files:upload_failed"))
				: task.mode === "chunked" && task.status === "uploading"
					? t("files:upload_chunk_status", {
							current: task.completedChunks ?? 0,
							total: task.totalChunks ?? 0,
						})
					: statusLabel;

		const actions =
			task.status === "failed"
				? [
						{
							label: t("files:upload_retry"),
							icon: "ArrowsClockwise" as const,
							onClick: () => retryTask(task.id),
							variant: "outline" as const,
						},
					]
				: ["queued", "initializing", "uploading", "processing"].includes(
							task.status,
						)
					? [
							{
								label: t("files:upload_dismiss"),
								icon: "X" as const,
								onClick: () => void cancelTask(task.id),
							},
						]
					: [];

		return {
			id: task.id,
			title: task.file.name,
			status: statusLabel,
			mode: modeLabel,
			progress: task.progress,
			detail,
			completed: task.status === "completed",
			actions,
		};
	});

	return (
		// biome-ignore lint/a11y/noStaticElementInteractions: drop zone
		<div
			className="relative flex flex-1 flex-col overflow-hidden"
			onDragEnter={handleDragEnter}
			onDragLeave={handleDragLeave}
			onDragOver={handleDragOver}
			onDrop={handleDrop}
		>
			{children}

			{uploadTasks.length > 0 && (
				<UploadPanel
					open={uploadPanelOpen}
					onToggle={() => setUploadPanelOpen((prev) => !prev)}
					title={t("files:upload")}
					summary={t("common:selected_count", { count: uploadTasks.length })}
					tasks={uploadTasks}
					emptyText={t("common:no_data")}
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
						{t("files:drop_files")}
					</p>
					<p className="mt-1 text-sm text-muted-foreground">
						{t("files:drop_files_desc")}
					</p>
				</div>
			)}
		</div>
	);
}
