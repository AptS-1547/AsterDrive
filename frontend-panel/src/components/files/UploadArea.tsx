import type { DragEvent, ReactNode } from "react";
import {
	forwardRef,
	useCallback,
	useEffect,
	useEffectEvent,
	useImperativeHandle,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import {
	UploadPanel,
	type UploadTaskView,
} from "@/components/files/UploadPanel";
import {
	CHUNK_PROCESSING_PROGRESS,
	getProcessingProgress,
	getResumePlan,
	type UploadMode,
} from "@/components/files/uploadResume";
import { Icon } from "@/components/ui/icon";
import { formatBytes } from "@/lib/format";
import {
	appendCompletedPart,
	loadSessions,
	type ResumableSession,
	removeSession,
	saveSession,
} from "@/lib/uploadPersistence";
import { cn } from "@/lib/utils";
import { workspaceKey } from "@/lib/workspace";
import { ApiError, api } from "@/services/http";
import {
	buildUploadPath,
	type CompletedPart,
	type InitUploadResponse,
	isRetryableUploadError,
	uploadService,
} from "@/services/uploadService";
import { useAuthStore } from "@/stores/authStore";
import { useFileStore } from "@/stores/fileStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import { ErrorCode } from "@/types/api-helpers";
import {
	extractFilesFromDrop,
	extractFilesFromInput,
	type FileWithPath,
	hasDirectoryInDropItems,
} from "@/utils/directoryUtils";

interface UploadAreaProps {
	children: ReactNode;
}

export interface UploadAreaHandle {
	triggerFileUpload: () => void;
	triggerFolderUpload: () => void;
}

type UploadStatus =
	| "pending_file"
	| "queued"
	| "initializing"
	| "uploading"
	| "processing"
	| "completed"
	| "failed"
	| "cancelled";

interface UploadTask {
	id: string;
	file: File | null;
	filename: string;
	relativePath: string | null;
	baseFolderId: number | null;
	baseFolderName: string;
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
const PROGRESS_FLUSH_INTERVAL = 500;
const RESTORE_PROGRESS_CONCURRENCY = 4;

function shouldRemovePersistedSession(error: unknown): boolean {
	return (
		error instanceof ApiError &&
		(error.code === ErrorCode.UploadSessionNotFound ||
			error.code === ErrorCode.UploadSessionExpired)
	);
}

async function mapAllSettledWithConcurrency<T, R>(
	items: readonly T[],
	concurrency: number,
	mapper: (item: T) => Promise<R>,
): Promise<PromiseSettledResult<R>[]> {
	const batchSize = Math.max(1, concurrency);
	const results: PromiseSettledResult<R>[] = [];

	for (let start = 0; start < items.length; start += batchSize) {
		const batch = items.slice(start, start + batchSize);
		results.push(...(await Promise.allSettled(batch.map(mapper))));
	}

	return results;
}

/** completeUpload with polling retry when backend is still assembling (3011) */
async function completeWithRetry(
	uploadId: string,
	parts?: import("@/services/uploadService").CompletedPart[],
): Promise<import("@/types/api").FileInfo> {
	const MAX_POLL = 30; // up to ~5 minutes
	const POLL_INTERVAL_MS = 10_000;
	for (let i = 0; i < MAX_POLL; i++) {
		try {
			return await uploadService.completeUpload(uploadId, parts);
		} catch (err) {
			if (
				err instanceof ApiError &&
				err.code === ErrorCode.UploadAssembling &&
				i < MAX_POLL - 1
			) {
				await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
				continue;
			}
			throw err;
		}
	}
	throw new Error("Upload timed out waiting for assembly");
}

function createTaskId() {
	return `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export const UploadArea = forwardRef<UploadAreaHandle, UploadAreaProps>(
	function UploadArea({ children }, ref) {
		const { t } = useTranslation(["core", "files"]);
		const refresh = useFileStore((s) => s.refresh);
		const currentFolderId = useFileStore((s) => s.currentFolderId);
		const breadcrumb = useFileStore((s) => s.breadcrumb);
		const workspace = useWorkspaceStore((s) => s.workspace);
		const refreshUser = useAuthStore((s) => s.refreshUser);
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
		const fileInputRef = useRef<HTMLInputElement | null>(null);
		const folderInputRef = useRef<HTMLInputElement | null>(null);
		const resumeFileInputRef = useRef<HTMLInputElement | null>(null);
		const resumeTaskIdRef = useRef<string | null>(null);
		const restoredWorkspaceKeysRef = useRef(new Set<string>());

		useImperativeHandle(
			ref,
			() => ({
				triggerFileUpload: () => fileInputRef.current?.click(),
				triggerFolderUpload: () => folderInputRef.current?.click(),
			}),
			[],
		);

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
				if (progressFlushTimerRef.current !== null) {
					window.clearTimeout(progressFlushTimerRef.current);
				}
			};
		}, []);

		const getRefreshBucketKey = useCallback((task: UploadTask) => {
			const topLevelSegment = task.relativePath?.split("/")[0] ?? "__file__";
			return `${task.baseFolderId ?? "root"}:${topLevelSegment}`;
		}, []);

		const scheduleRefresh = useCallback(
			(baseFolderId: number | null) => {
				if (refreshTimeoutRef.current !== null) return;
				refreshTimeoutRef.current = window.setTimeout(() => {
					refreshTimeoutRef.current = null;
					void refreshUser();
					if (currentFolderIdRef.current === baseFolderId) {
						void refresh();
					}
				}, 300);
			},
			[refresh, refreshUser],
		);

		const patchTask = useCallback(
			(taskId: string, patch: Partial<UploadTask>) => {
				const terminalStatus: UploadStatus[] = ["completed", "cancelled"];
				const finalPatch =
					patch.status && terminalStatus.includes(patch.status)
						? { ...patch, file: null }
						: patch;
				setTasks((prev) =>
					prev.map((task) =>
						task.id === taskId ? { ...task, ...finalPatch } : task,
					),
				);
			},
			[],
		);

		// ── 节流进度更新：累积到 buffer，每 500ms 批量 flush ──
		const progressBufferRef = useRef(new Map<string, Partial<UploadTask>>());
		const progressFlushTimerRef = useRef<number | null>(null);

		const flushProgress = useCallback(() => {
			progressFlushTimerRef.current = null;
			const buffer = progressBufferRef.current;
			if (buffer.size === 0) return;
			const updates = new Map(buffer);
			buffer.clear();
			setTasks((prev) =>
				prev.map((task) => {
					const patch = updates.get(task.id);
					return patch ? { ...task, ...patch } : task;
				}),
			);
		}, []);

		/** 节流版 patchTask — 仅用于 progress/completedChunks 等高频更新 */
		const patchTaskThrottled = useCallback(
			(taskId: string, patch: Partial<UploadTask>) => {
				const existing = progressBufferRef.current.get(taskId);
				progressBufferRef.current.set(
					taskId,
					existing ? { ...existing, ...patch } : patch,
				);
				if (progressFlushTimerRef.current === null) {
					progressFlushTimerRef.current = window.setTimeout(
						flushProgress,
						PROGRESS_FLUSH_INTERVAL,
					);
				}
			},
			[flushProgress],
		);

		const finalizeTaskRefresh = useCallback(
			(task: UploadTask) => {
				const bucketKey = getRefreshBucketKey(task);
				const hasPendingInBucket = tasksRef.current.some((candidate) => {
					if (candidate.id === task.id) return false;
					return (
						getRefreshBucketKey(candidate) === bucketKey &&
						["queued", "initializing", "uploading", "processing"].includes(
							candidate.status,
						)
					);
				});
				if (!hasPendingInBucket) {
					scheduleRefresh(task.baseFolderId);
				}
			},
			[getRefreshBucketKey, scheduleRefresh],
		);

		const clearCompletedTasks = useCallback(() => {
			setTasks((prev) => prev.filter((task) => task.status !== "completed"));
		}, []);

		const restorePendingSessions = useEffectEvent(async () => {
			const currentWorkspaceKey = workspaceKey(workspace);
			if (restoredWorkspaceKeysRef.current.has(currentWorkspaceKey)) {
				return;
			}
			restoredWorkspaceKeysRef.current.add(currentWorkspaceKey);

			const sessions = loadSessions(workspace);
			if (sessions.length === 0) return;

			const ghostTasks: UploadTask[] = [];
			const completionTasks: Array<{
				task: UploadTask;
				parts?: CompletedPart[];
			}> = [];

			const progressResults = await mapAllSettledWithConcurrency(
				sessions,
				RESTORE_PROGRESS_CONCURRENCY,
				async (session) => {
					try {
						const progress = await uploadService.getProgress(session.uploadId);
						return { session, progress };
					} catch (error) {
						throw { session, error };
					}
				},
			);

			for (const result of progressResults) {
				if (result.status === "rejected") {
					const { session, error } = result.reason as {
						session: ResumableSession;
						error: unknown;
					};
					if (shouldRemovePersistedSession(error)) {
						removeSession(session.uploadId);
					}
					continue;
				}

				const { session, progress } = result.value;
				if (!progress?.status) {
					if (process.env.NODE_ENV === "development") {
						console.warn(
							"skipping restored upload session because progress is missing a status",
							{
								progress,
								uploadId: session.uploadId,
							},
						);
					}
					continue;
				}
				const mode = (session.mode ?? "chunked") as UploadMode;
				const plan = getResumePlan(mode, progress.status);
				if (plan === "restart") {
					removeSession(session.uploadId);
					if (progress.status === "failed") {
						ghostTasks.push({
							id: createTaskId(),
							file: null,
							filename: session.filename,
							relativePath: session.relativePath,
							baseFolderId: session.baseFolderId,
							baseFolderName: session.baseFolderName,
							mode,
							status: "pending_file",
							progress: 0,
							error: t("files:upload_failed"),
							uploadId: null,
							totalChunks: session.totalChunks,
							completedChunks: progress.received_count,
						});
					}
					continue;
				}

				const task: UploadTask = {
					id: createTaskId(),
					file: null,
					filename: session.filename,
					relativePath: session.relativePath,
					baseFolderId: session.baseFolderId,
					baseFolderName: session.baseFolderName,
					mode,
					status: plan === "upload" ? "pending_file" : "processing",
					progress: plan === "upload" ? 0 : getProcessingProgress(mode),
					error: null,
					uploadId: session.uploadId,
					totalChunks: session.totalChunks,
					completedChunks:
						plan === "upload" ? progress.received_count : session.totalChunks,
				};
				ghostTasks.push(task);
				if (plan === "complete") {
					completionTasks.push({
						task,
						parts:
							mode === "presigned_multipart"
								? (session.completedParts ?? [])
								: undefined,
					});
				}
			}

			if (ghostTasks.length > 0) {
				setTasks((prev) => [...ghostTasks, ...prev]);
				setUploadPanelOpen(true);
				for (const completionTask of completionTasks) {
					void resumeCompletionTask(completionTask.task, completionTask.parts);
				}
			}
		});

		// ── 断点续传：mount 时恢复未完成 session ──
		useEffect(() => {
			const currentWorkspaceKey = workspaceKey(workspace);
			if (restoredWorkspaceKeysRef.current.has(currentWorkspaceKey)) {
				return;
			}
			void restorePendingSessions();
		}, [workspace]);

		/** 用户为 pending_file task 选好文件后注入 File → 转为 queued */
		const attachFileToTask = useCallback(
			(taskId: string, file: File) => {
				const task = tasksRef.current.find((item) => item.id === taskId);
				if (!task || task.status !== "pending_file") return;

				// 校验 name + size 匹配
				const sessions = loadSessions(workspace);
				const session = sessions.find((s) => s.uploadId === task.uploadId);
				if (
					session &&
					(file.name !== session.filename || file.size !== session.totalSize)
				) {
					patchTask(taskId, {
						error: t("files:upload_resume_mismatch", {
							name: session.filename,
							size: formatBytes(session.totalSize),
						}),
					});
					return;
				}

				patchTask(taskId, {
					file,
					status: "queued",
					error: null,
					progress: 0,
				});
			},
			[patchTask, t, workspace],
		);

		const handleResumeFileChange = useCallback(
			(event: React.ChangeEvent<HTMLInputElement>) => {
				const files = event.target.files;
				const taskId = resumeTaskIdRef.current;
				if (!files?.[0] || !taskId) return;
				attachFileToTask(taskId, files[0]);
				event.target.value = "";
				resumeTaskIdRef.current = null;
			},
			[attachFileToTask],
		);

		const triggerResumeFilePicker = useCallback((taskId: string) => {
			resumeTaskIdRef.current = taskId;
			resumeFileInputRef.current?.click();
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

		const resumeCompletionTask = useCallback(
			async (task: UploadTask, parts?: CompletedPart[]) => {
				const uploadId = task.uploadId;
				if (!uploadId) return;

				abortFlagsRef.current.set(task.id, false);
				patchTask(task.id, {
					status: "processing",
					progress: getProcessingProgress(task.mode),
				});

				try {
					await completeWithRetry(uploadId, parts);
					if (abortFlagsRef.current.get(task.id)) {
						patchTask(task.id, { status: "cancelled", error: null });
						return;
					}
					removeSession(uploadId);
					patchTask(task.id, {
						status: "completed",
						progress: 100,
						error: null,
					});
					finalizeTaskRefresh(task);
				} catch (error) {
					if (abortFlagsRef.current.get(task.id)) {
						patchTask(task.id, { status: "cancelled", error: null });
						return;
					}
					const message =
						error instanceof Error
							? error.message
							: t("errors:unexpected_error");
					if (!task.file) {
						if (shouldRemovePersistedSession(error)) {
							removeSession(uploadId);
							patchTask(task.id, {
								status: "pending_file",
								error: message,
								progress: 0,
								uploadId: null,
								completedChunks: 0,
								totalChunks: 0,
								mode: null,
							});
							return;
						}
						markTaskFailed(task.id, message);
						return;
					}
					markTaskFailed(task.id, message);
				} finally {
					abortFlagsRef.current.delete(task.id);
				}
			},
			[finalizeTaskRefresh, markTaskFailed, patchTask, t],
		);

		const buildDirectUploadPath = useCallback(
			(task: UploadTask) => {
				const params = new URLSearchParams();
				const folderId = task.baseFolderId;
				if (folderId !== null) {
					params.set("folder_id", String(folderId));
				}
				if (task.relativePath) {
					params.set("relative_path", task.relativePath);
				}
				const basePath = buildUploadPath(workspace, "/files/upload");
				const query = params.toString();
				return query ? `${basePath}?${query}` : basePath;
			},
			[workspace],
		);

		const runDirectUpload = useCallback(
			async (task: UploadTask) => {
				if (!task.file) return;
				patchTask(task.id, {
					mode: "direct",
					status: "uploading",
					progress: 0,
				});
				const controller = new AbortController();
				directAbortRef.current.set(task.id, controller);

				try {
					const formData = new FormData();
					formData.append("file", task.file);
					await api.client.post(buildDirectUploadPath(task), formData, {
						headers: { "Content-Type": "multipart/form-data" },
						signal: controller.signal,
						onUploadProgress: (event) => {
							if (!event.total) return;
							patchTaskThrottled(task.id, {
								progress: Math.round((event.loaded / event.total) * 100),
							});
						},
					});

					patchTask(task.id, {
						status: "completed",
						progress: 100,
						error: null,
					});
					finalizeTaskRefresh(task);
				} catch (error) {
					if (controller.signal.aborted) {
						patchTask(task.id, { status: "cancelled", error: null });
						return;
					}
					const message =
						error instanceof Error
							? error.message
							: t("errors:unexpected_error");
					markTaskFailed(task.id, message);
				} finally {
					directAbortRef.current.delete(task.id);
				}
			},
			[
				buildDirectUploadPath,
				finalizeTaskRefresh,
				markTaskFailed,
				patchTask,
				patchTaskThrottled,
				t,
			],
		);

		const runChunkedUpload = useCallback(
			async (
				task: UploadTask,
				init: InitUploadResponse,
				alreadyReceived: number[] = [],
			) => {
				if (!task.file) return;
				const file = task.file;
				const uploadId = init.upload_id as string;
				const chunkSize = init.chunk_size as number;
				const totalChunks = init.total_chunks as number;
				abortFlagsRef.current.set(task.id, false);
				patchTask(task.id, {
					mode: "chunked",
					status: "uploading",
					uploadId,
					totalChunks,
					completedChunks: alreadyReceived.length,
					progress: Math.round((alreadyReceived.length / totalChunks) * 95),
				});

				let completed = alreadyReceived.length;
				const queue = Array.from(
					{ length: totalChunks },
					(_, index) => index,
				).filter((index) => !alreadyReceived.includes(index));

				const uploadOneChunk = async () => {
					while (queue.length > 0) {
						if (abortFlagsRef.current.get(task.id)) return;
						const chunkNumber = queue.shift();
						if (chunkNumber === undefined) return;
						const start = chunkNumber * chunkSize;
						const end = Math.min(start + chunkSize, file.size);
						const blob = file.slice(start, end);

						let lastError: Error | null = null;
						for (let attempt = 0; attempt < CHUNK_MAX_RETRIES; attempt++) {
							try {
								await uploadService.uploadChunk(uploadId, chunkNumber, blob);
								lastError = null;
								break;
							} catch (error) {
								lastError =
									error instanceof Error ? error : new Error(String(error));
								if (!isRetryableUploadError(lastError)) {
									break;
								}
								if (attempt < CHUNK_MAX_RETRIES - 1) {
									await new Promise((resolve) =>
										setTimeout(resolve, 1000 * 2 ** attempt),
									);
								}
							}
						}

						if (lastError) throw lastError;
						completed += 1;
						patchTaskThrottled(task.id, {
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

					flushProgress();
					patchTask(task.id, {
						status: "processing",
						progress: CHUNK_PROCESSING_PROGRESS,
					});
					await completeWithRetry(uploadId);
					removeSession(uploadId);
					patchTask(task.id, {
						status: "completed",
						progress: 100,
						error: null,
					});
					finalizeTaskRefresh(task);
				} catch (error) {
					if (abortFlagsRef.current.get(task.id)) {
						patchTask(task.id, { status: "cancelled", error: null });
						return;
					}
					const message =
						error instanceof Error
							? error.message
							: t("errors:unexpected_error");
					markTaskFailed(task.id, message);
				} finally {
					abortFlagsRef.current.delete(task.id);
				}
			},
			[
				finalizeTaskRefresh,
				flushProgress,
				markTaskFailed,
				patchTask,
				patchTaskThrottled,
				t,
			],
		);

		const runPresignedUpload = useCallback(
			async (task: UploadTask, init: InitUploadResponse) => {
				if (!task.file) return;
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
							patchTaskThrottled(task.id, {
								progress: Math.round((loaded / total) * 90),
							});
						},
						(xhr) => {
							presignedXhrRef.current.set(task.id, xhr);
						},
					);

					flushProgress();
					patchTask(task.id, {
						status: "processing",
						progress: getProcessingProgress(task.mode),
					});
					await completeWithRetry(uploadId);
					patchTask(task.id, {
						status: "completed",
						progress: 100,
						error: null,
					});
					finalizeTaskRefresh(task);
				} catch (error) {
					const message =
						error instanceof Error
							? error.message
							: t("errors:unexpected_error");
					if (message.includes("abort")) {
						patchTask(task.id, { status: "cancelled", error: null });
						return;
					}
					markTaskFailed(task.id, message);
				} finally {
					presignedXhrRef.current.delete(task.id);
				}
			},
			[
				finalizeTaskRefresh,
				flushProgress,
				markTaskFailed,
				patchTask,
				patchTaskThrottled,
				t,
			],
		);

		const runMultipartUpload = useCallback(
			async (
				task: UploadTask,
				init: InitUploadResponse,
				alreadyCompleted: CompletedPart[] = [],
			) => {
				if (!task.file) return;
				const file = task.file;
				const uploadId = init.upload_id as string;
				const chunkSize = init.chunk_size as number;
				const totalChunks = init.total_chunks as number;
				abortFlagsRef.current.set(task.id, false);

				const collectedParts: CompletedPart[] = [...alreadyCompleted];
				const completedSet = new Set(
					alreadyCompleted.map((p) => p.part_number),
				);

				patchTask(task.id, {
					mode: "presigned_multipart" as UploadMode,
					status: "uploading",
					uploadId,
					totalChunks,
					completedChunks: completedSet.size,
					progress: Math.round((completedSet.size / totalChunks) * 90),
				});

				// S3 multipart parts are 1-indexed
				const queue = Array.from(
					{ length: totalChunks },
					(_, i) => i + 1,
				).filter((n) => !completedSet.has(n));

				// 分批请求 presigned URLs（每批最多 50 个）
				const BATCH_SIZE = 50;
				let urlCache: Record<number, string> = {};

				const getPartUrl = async (partNum: number): Promise<string> => {
					if (urlCache[partNum]) return urlCache[partNum];
					// 预取当前 part 后面的一批
					const idx = queue.indexOf(partNum);
					const batch =
						idx >= 0 ? queue.slice(idx, idx + BATCH_SIZE) : [partNum];
					const urls = await uploadService.presignParts(uploadId, batch);
					urlCache = { ...urlCache, ...urls };
					return urlCache[partNum];
				};

				let completed = completedSet.size;

				const uploadOnePart = async () => {
					while (queue.length > 0) {
						if (abortFlagsRef.current.get(task.id)) return;
						const partNum = queue.shift();
						if (partNum === undefined) return;

						const start = (partNum - 1) * chunkSize;
						const end = Math.min(start + chunkSize, file.size);
						const blob = file.slice(start, end);

						let lastError: Error | null = null;
						for (let attempt = 0; attempt < CHUNK_MAX_RETRIES; attempt++) {
							try {
								const url = await getPartUrl(partNum);
								const etag = await uploadService.presignedUpload(url, blob);
								const part: CompletedPart = {
									part_number: partNum,
									etag: etag.replace(/"/g, ""),
								};
								collectedParts.push(part);
								appendCompletedPart(uploadId, part);
								lastError = null;
								break;
							} catch (error) {
								lastError =
									error instanceof Error ? error : new Error(String(error));
								// URL 可能过期，清缓存重新获取
								delete urlCache[partNum];
								if (!isRetryableUploadError(lastError)) {
									break;
								}
								if (attempt < CHUNK_MAX_RETRIES - 1) {
									await new Promise((resolve) =>
										setTimeout(resolve, 1000 * 2 ** attempt),
									);
								}
							}
						}

						if (lastError) throw lastError;
						completed += 1;
						patchTaskThrottled(task.id, {
							completedChunks: completed,
							progress: Math.round((completed / totalChunks) * 90),
						});
					}
				};

				try {
					const workers = Array.from(
						{ length: Math.min(CHUNK_CONCURRENT, queue.length || 1) },
						() => uploadOnePart(),
					);
					await Promise.all(workers);

					if (abortFlagsRef.current.get(task.id)) {
						patchTask(task.id, { status: "cancelled", error: null });
						return;
					}

					flushProgress();
					patchTask(task.id, {
						status: "processing",
						progress: getProcessingProgress(task.mode),
					});

					// 按 part_number 排序后发送
					collectedParts.sort((a, b) => a.part_number - b.part_number);
					await completeWithRetry(uploadId, collectedParts);
					removeSession(uploadId);
					patchTask(task.id, {
						status: "completed",
						progress: 100,
						error: null,
					});
					finalizeTaskRefresh(task);
				} catch (error) {
					if (abortFlagsRef.current.get(task.id)) {
						patchTask(task.id, { status: "cancelled", error: null });
						return;
					}
					const message =
						error instanceof Error
							? error.message
							: t("errors:unexpected_error");
					markTaskFailed(task.id, message);
				} finally {
					abortFlagsRef.current.delete(task.id);
				}
			},
			[
				finalizeTaskRefresh,
				flushProgress,
				markTaskFailed,
				patchTask,
				patchTaskThrottled,
				t,
			],
		);

		const runTask = useCallback(
			async (taskId: string) => {
				const task = tasksRef.current.find((item) => item.id === taskId);
				if (!task || task.status !== "queued" || !task.file) return;
				const file = task.file;

				patchTask(taskId, { status: "initializing", error: null, progress: 0 });
				try {
					// ── Resume 路径：chunked 或 presigned_multipart 有 uploadId ──
					if (
						task.uploadId &&
						(task.mode === "chunked" || task.mode === "presigned_multipart")
					) {
						try {
							const progress = await uploadService.getProgress(task.uploadId);
							const plan = getResumePlan(task.mode, progress.status);
							if (plan === "restart") {
								removeSession(task.uploadId);
								patchTask(taskId, {
									uploadId: null,
									completedChunks: 0,
									totalChunks: 0,
									mode: null,
								});
							}
							if (plan !== "restart") {
								const saved = loadSessions(workspace).find(
									(session) => session.uploadId === task.uploadId,
								);
								if (plan === "complete") {
									await resumeCompletionTask(
										task,
										task.mode === "presigned_multipart"
											? (saved?.completedParts ?? [])
											: undefined,
									);
									return;
								}
								const chunkSize =
									(
										progress as typeof progress & {
											chunk_size?: number;
										}
									).chunk_size ?? saved?.chunkSize;
								if (!chunkSize || chunkSize <= 0) {
									throw new Error("missing resumable chunk size");
								}
								if (task.mode === "chunked") {
									const resumedInit: InitUploadResponse = {
										mode: "chunked",
										upload_id: task.uploadId,
										chunk_size: chunkSize,
										total_chunks: progress.total_chunks,
									};
									await runChunkedUpload(
										task,
										resumedInit,
										progress.chunks_on_disk,
									);
								} else {
									// presigned_multipart resume — 从 localStorage 读已完成 parts
									const resumedInit: InitUploadResponse = {
										mode: "presigned_multipart",
										upload_id: task.uploadId,
										chunk_size: chunkSize,
										total_chunks: progress.total_chunks,
									};
									await runMultipartUpload(
										task,
										resumedInit,
										saved?.completedParts ?? [],
									);
								}
								return;
							}
						} catch (error) {
							if (shouldRemovePersistedSession(error)) {
								removeSession(task.uploadId);
								patchTask(taskId, {
									uploadId: null,
									completedChunks: 0,
									totalChunks: 0,
									mode: null,
								});
							} else {
								const message =
									error instanceof Error
										? error.message
										: t("errors:unexpected_error");
								markTaskFailed(taskId, message);
								return;
							}
						}
					}

					// ── Fresh init 路径 ──
					const init = await uploadService.initUpload({
						filename: file.name,
						total_size: file.size,
						folder_id: task.baseFolderId,
						relative_path: task.relativePath ?? undefined,
					});
					// 持久化可恢复的 session（chunked 和 presigned_multipart）
					if (
						(init.mode === "chunked" || init.mode === "presigned_multipart") &&
						init.upload_id
					) {
						saveSession({
							uploadId: init.upload_id,
							filename: file.name,
							totalSize: file.size,
							totalChunks: init.total_chunks ?? 0,
							chunkSize: init.chunk_size ?? 0,
							baseFolderId: task.baseFolderId,
							baseFolderName: task.baseFolderName,
							relativePath: task.relativePath,
							savedAt: Date.now(),
							workspace,
							mode:
								init.mode === "presigned_multipart"
									? "presigned_multipart"
									: "chunked",
						});
					}
					if (init.mode === "chunked") {
						await runChunkedUpload(task, init);
					} else if (init.mode === "presigned_multipart") {
						await runMultipartUpload(task, init);
					} else if (init.mode === "presigned") {
						await runPresignedUpload(task, init);
					} else {
						await runDirectUpload(task);
					}
				} catch (error) {
					const message =
						error instanceof Error
							? error.message
							: t("errors:unexpected_error");
					markTaskFailed(taskId, message);
					finalizeTaskRefresh(task);
				}
			},
			[
				finalizeTaskRefresh,
				markTaskFailed,
				patchTask,
				runChunkedUpload,
				runDirectUpload,
				runMultipartUpload,
				runPresignedUpload,
				resumeCompletionTask,
				t,
				workspace,
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

				// pending_file 取消：只需移除持久化 + task
				if (task.status === "pending_file") {
					if (task.uploadId) removeSession(task.uploadId);
					setTasks((prev) => prev.filter((item) => item.id !== taskId));
					return;
				}

				abortFlagsRef.current.set(taskId, true);
				if (task.uploadId) {
					try {
						await uploadService.cancelUpload(task.uploadId);
					} catch {
						// ignore
					}
					removeSession(task.uploadId);
				}
				patchTask(taskId, { status: "cancelled", error: null });
			},
			[patchTask],
		);

		const retryTask = useCallback(
			(taskId: string) => {
				const task = tasksRef.current.find((item) => item.id === taskId);
				if (!task) return;
				if (!task.file && task.uploadId) {
					const saved = loadSessions(workspace).find(
						(session) => session.uploadId === task.uploadId,
					);
					void resumeCompletionTask(
						task,
						task.mode === "presigned_multipart"
							? (saved?.completedParts ?? [])
							: undefined,
					);
					setUploadPanelOpen(true);
					return;
				}
				if (task.uploadId) {
					void uploadService.cancelUpload(task.uploadId).catch(() => undefined);
					removeSession(task.uploadId);
				}
				patchTask(taskId, {
					status: "queued",
					progress: 0,
					error: null,
					uploadId: null,
					completedChunks: 0,
					totalChunks: 0,
					mode: null,
				});
				setUploadPanelOpen(true);
			},
			[patchTask, resumeCompletionTask, workspace],
		);

		const retryFailedTasks = useCallback(() => {
			const failedTaskIds = tasksRef.current
				.filter((task) => task.status === "failed")
				.map((task) => task.id);
			for (const taskId of failedTaskIds) {
				retryTask(taskId);
			}
		}, [retryTask]);

		const addFilesWithPath = useCallback(
			(files: FileWithPath[]) => {
				if (files.length === 0) return;
				const baseFolderId = currentFolderIdRef.current;
				const baseFolderName =
					breadcrumb[breadcrumb.length - 1]?.name ?? t("files:root");
				const nextTasks = files.map(({ file, relativePath }) => ({
					id: createTaskId(),
					file,
					filename: file.name,
					relativePath,
					baseFolderId,
					baseFolderName,
					mode: null,
					status: "queued" as UploadStatus,
					progress: 0,
					error: null,
					uploadId: null,
				}));
				setTasks((prev) => [...nextTasks, ...prev]);
				setUploadPanelOpen(true);
			},
			[breadcrumb, t],
		);

		const addFiles = useCallback(
			(files: FileList | null) => {
				if (!files || files.length === 0) return;
				const baseFolderId = currentFolderIdRef.current;
				const baseFolderName =
					breadcrumb[breadcrumb.length - 1]?.name ?? t("files:root");
				const nextTasks = Array.from(files).map((file) => ({
					id: createTaskId(),
					file,
					filename: file.name,
					relativePath: null,
					baseFolderId,
					baseFolderName,
					mode: null,
					status: "queued" as UploadStatus,
					progress: 0,
					error: null,
					uploadId: null,
				}));
				setTasks((prev) => [...nextTasks, ...prev]);
				setUploadPanelOpen(true);
			},
			[breadcrumb, t],
		);

		const handleFileInputChange = useCallback(
			(event: React.ChangeEvent<HTMLInputElement>) => {
				addFiles(event.target.files);
				event.target.value = "";
			},
			[addFiles],
		);

		const handleFolderInputChange = useCallback(
			(event: React.ChangeEvent<HTMLInputElement>) => {
				const files = event.target.files;
				if (!files) return;
				addFilesWithPath(extractFilesFromInput(files));
				event.target.value = "";
			},
			[addFilesWithPath],
		);

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
		const handleDrop = async (e: DragEvent<HTMLDivElement>) => {
			e.preventDefault();
			dragCounter.current = 0;
			setIsDragging(false);
			if (
				e.dataTransfer.items?.length &&
				(e.dataTransfer.files.length === 0 ||
					hasDirectoryInDropItems(e.dataTransfer.items))
			) {
				const files = await extractFilesFromDrop(e.dataTransfer.items);
				addFilesWithPath(files);
				return;
			}
			addFiles(e.dataTransfer.files);
		};

		// ── 统计：单次遍历代替 4 次 filter + 1 次 reduce ──
		let successCount = 0;
		let failedCount = 0;
		let activeCount = 0;
		let progressSum = 0;
		for (const task of tasks) {
			progressSum += task.progress;
			if (task.status === "completed") successCount++;
			else if (task.status === "failed") failedCount++;
			else if (
				task.status === "queued" ||
				task.status === "initializing" ||
				task.status === "uploading" ||
				task.status === "processing"
			)
				activeCount++;
		}
		const totalCount = tasks.length;
		const overallProgress =
			totalCount === 0 ? 0 : Math.round(progressSum / totalCount);

		const uploadTasks: UploadTaskView[] = tasks.map((task) => {
			const isPendingFile = task.status === "pending_file";

			const taskTitle = task.filename;

			const modeLabel =
				task.mode === "chunked"
					? "Chunked"
					: task.mode === "presigned"
						? "S3"
						: task.mode === "presigned_multipart"
							? "S3 Chunked"
							: task.mode === "direct"
								? "Direct"
								: "Pending";

			const statusLabel = isPendingFile
				? t("files:upload_pending_file")
				: task.status === "queued"
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

			const detail = isPendingFile
				? (task.error ??
					t("files:upload_chunk_status", {
						current: task.completedChunks ?? 0,
						total: task.totalChunks ?? 0,
					}))
				: task.status === "failed"
					? (task.error ?? t("files:upload_failed"))
					: (task.mode === "chunked" || task.mode === "presigned_multipart") &&
							task.status === "uploading"
						? t("files:upload_chunk_status", {
								current: task.completedChunks ?? 0,
								total: task.totalChunks ?? 0,
							})
						: task.relativePath || statusLabel;

			const actions = isPendingFile
				? [
						{
							label: t("files:upload_resume_select"),
							icon: "Upload" as const,
							onClick: () => triggerResumeFilePicker(task.id),
							variant: "outline" as const,
						},
						{
							label: t("files:upload_dismiss"),
							icon: "X" as const,
							onClick: () => void cancelTask(task.id),
						},
					]
				: task.status === "failed"
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

			const group = task.relativePath
				? task.relativePath.split("/").slice(0, -1).join("/")
				: "";

			return {
				id: task.id,
				title: taskTitle,
				status: statusLabel,
				mode: modeLabel,
				progress: task.progress,
				group,
				targetLabel: task.baseFolderName,
				detail,
				completed: task.status === "completed",
				actions,
			};
		});

		return (
			<>
				<input
					ref={fileInputRef}
					type="file"
					multiple
					className="hidden"
					onChange={handleFileInputChange}
				/>
				<input
					ref={folderInputRef}
					type="file"
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
					onDrop={(e) => {
						void handleDrop(e);
					}}
				>
					{children}

					{uploadTasks.length > 0 && (
						<UploadPanel
							open={uploadPanelOpen}
							onToggle={() => setUploadPanelOpen((prev) => !prev)}
							title={t("files:upload")}
							summary={t("files:upload_summary", {
								total: totalCount,
								success: successCount,
								failed: failedCount,
								active: activeCount,
							})}
							tasks={uploadTasks}
							emptyText={t("no_data")}
							totalCount={totalCount}
							successCount={successCount}
							failedCount={failedCount}
							activeCount={activeCount}
							overallProgress={overallProgress}
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
