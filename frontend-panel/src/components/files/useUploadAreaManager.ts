import type { ChangeEvent, DragEvent, RefObject } from "react";
import {
	useCallback,
	useEffect,
	useEffectEvent,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import type { UploadTaskView } from "@/components/files/UploadPanel";
import {
	CHUNK_PROCESSING_PROGRESS,
	getProcessingProgress,
	getResumePlan,
	type UploadMode,
} from "@/components/files/uploadResume";
import { formatBytes } from "@/lib/format";
import { logger } from "@/lib/logger";
import {
	clearDeferredStorageRefresh,
	consumeDeferredStorageRefresh,
	enterStorageRefreshGate,
	leaveStorageRefreshGate,
} from "@/lib/storageRefreshGate";
import {
	appendCompletedPart,
	loadSessions,
	type ResumableSession,
	removeSession,
	saveSession,
} from "@/lib/uploadPersistence";
import type { Workspace } from "@/lib/workspace";
import { workspaceKey } from "@/lib/workspace";
import { ApiError, api } from "@/services/http";
import {
	buildUploadPath,
	type CompletedPart,
	type InitUploadResponse,
	isRetryableUploadError,
	uploadService,
} from "@/services/uploadService";
import { ErrorCode } from "@/types/api-helpers";
import {
	extractFilesFromDrop,
	extractFilesFromInput,
	type FileWithPath,
	hasDirectoryInDropItems,
} from "@/utils/directoryUtils";

type UploadStatus =
	| "pending_file"
	| "queued"
	| "initializing"
	| "uploading"
	| "processing"
	| "completed"
	| "failed"
	| "cancelled";

const ACTIVE_QUEUE_STATUSES: UploadStatus[] = [
	"queued",
	"initializing",
	"uploading",
	"processing",
];

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
const MULTIPART_DRAIN_TIMEOUT_MS = 3000;
const MULTIPART_DRAIN_POLL_MS = 50;

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

async function completeWithRetry(
	uploadId: string,
	parts?: CompletedPart[],
): Promise<import("@/types/api").FileInfo> {
	const MAX_POLL = 30;
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
				await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
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

interface UseUploadAreaManagerOptions {
	breadcrumb: Array<{ id: number | null; name: string }>;
	currentFolderId: number | null;
	refresh: () => Promise<void>;
	refreshUser: () => Promise<void>;
	resumeFileInputRef: RefObject<HTMLInputElement | null>;
	workspace: Workspace;
}

export function useUploadAreaManager({
	breadcrumb,
	currentFolderId,
	refresh,
	refreshUser,
	resumeFileInputRef,
	workspace,
}: UseUploadAreaManagerOptions) {
	const { t } = useTranslation(["core", "files"]);
	const currentFolderIdRef = useRef(currentFolderId);
	const [isDragging, setIsDragging] = useState(false);
	const dragCounter = useRef(0);
	const [uploadPanelOpen, setUploadPanelOpen] = useState(true);
	const [tasks, setTasks] = useState<UploadTask[]>([]);
	const tasksRef = useRef<UploadTask[]>([]);
	const abortFlagsRef = useRef(new Map<string, boolean>());
	const directAbortRef = useRef(new Map<string, AbortController>());
	const presignedXhrRef = useRef(new Map<string, XMLHttpRequest>());
	const multipartInFlightRef = useRef(new Map<string, number>());
	const pendingRefreshFolderIdsRef = useRef(new Set<number | null>());
	const queueWasActiveRef = useRef(false);
	const resumeTaskIdRef = useRef<string | null>(null);
	const restoredWorkspaceKeysRef = useRef(new Set<string>());

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
			if (progressFlushTimerRef.current !== null) {
				window.clearTimeout(progressFlushTimerRef.current);
			}
			if (queueWasActiveRef.current) {
				leaveStorageRefreshGate();
				queueWasActiveRef.current = false;
				clearDeferredStorageRefresh();
			}
		};
	}, []);

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

	const markFolderForRefresh = useCallback((task: UploadTask) => {
		pendingRefreshFolderIdsRef.current.add(task.baseFolderId);
	}, []);

	useEffect(() => {
		const hasActiveQueue = tasks.some((task) =>
			ACTIVE_QUEUE_STATUSES.includes(task.status),
		);

		if (hasActiveQueue) {
			if (!queueWasActiveRef.current) {
				enterStorageRefreshGate();
			}
			queueWasActiveRef.current = true;
			return;
		}

		if (!queueWasActiveRef.current) {
			return;
		}

		leaveStorageRefreshGate();
		queueWasActiveRef.current = false;

		const pendingRefreshFolderIds = pendingRefreshFolderIdsRef.current;
		const hasDeferredRefresh = consumeDeferredStorageRefresh();
		if (pendingRefreshFolderIds.size === 0 && !hasDeferredRefresh) {
			return;
		}

		const shouldRefreshCurrentFolder =
			hasDeferredRefresh ||
			pendingRefreshFolderIds.has(currentFolderIdRef.current);
		pendingRefreshFolderIdsRef.current = new Set();

		if (pendingRefreshFolderIds.size > 0) {
			void refreshUser();
		}
		if (shouldRefreshCurrentFolder) {
			void refresh();
		}
	}, [refresh, refreshUser, tasks]);

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
					logger.warn(
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

	useEffect(() => {
		const currentWorkspaceKey = workspaceKey(workspace);
		if (restoredWorkspaceKeysRef.current.has(currentWorkspaceKey)) {
			return;
		}
		void restorePendingSessions();
	}, [workspace]);

	const attachFileToTask = useCallback(
		(taskId: string, file: File) => {
			const task = tasksRef.current.find((item) => item.id === taskId);
			if (!task || task.status !== "pending_file") return;

			const sessions = loadSessions(workspace);
			const session = sessions.find(
				(entry) => entry.uploadId === task.uploadId,
			);
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
		(event: ChangeEvent<HTMLInputElement>) => {
			const files = event.target.files;
			const taskId = resumeTaskIdRef.current;
			if (!files?.[0] || !taskId) return;
			attachFileToTask(taskId, files[0]);
			event.target.value = "";
			resumeTaskIdRef.current = null;
		},
		[attachFileToTask],
	);

	const triggerResumeFilePicker = useCallback(() => {
		resumeFileInputRef.current?.click();
	}, [resumeFileInputRef]);

	const requestResumeFilePicker = useCallback(
		(taskId: string) => {
			resumeTaskIdRef.current = taskId;
			triggerResumeFilePicker();
		},
		[triggerResumeFilePicker],
	);

	const markTaskFailed = useCallback(
		(taskId: string, message: string) => {
			patchTask(taskId, {
				status: "failed",
				error: message,
			});
		},
		[patchTask],
	);

	const adjustMultipartInFlight = useCallback(
		(taskId: string, delta: number) => {
			const current = multipartInFlightRef.current.get(taskId) ?? 0;
			const next = current + delta;
			if (next <= 0) {
				multipartInFlightRef.current.delete(taskId);
				return;
			}
			multipartInFlightRef.current.set(taskId, next);
		},
		[],
	);

	const waitForMultipartDrain = useCallback(async (taskId: string) => {
		const startedAt = Date.now();
		while ((multipartInFlightRef.current.get(taskId) ?? 0) > 0) {
			if (Date.now() - startedAt >= MULTIPART_DRAIN_TIMEOUT_MS) {
				return;
			}
			await new Promise((resolve) =>
				setTimeout(resolve, MULTIPART_DRAIN_POLL_MS),
			);
		}
	}, []);

	const cancelMultipartSession = useCallback(
		async (task: UploadTask) => {
			abortFlagsRef.current.set(task.id, true);
			if (!task.uploadId) return;

			await waitForMultipartDrain(task.id);
			try {
				await uploadService.cancelUpload(task.uploadId);
			} catch {}
			removeSession(task.uploadId);
		},
		[waitForMultipartDrain],
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
				markFolderForRefresh(task);
			} catch (error) {
				if (abortFlagsRef.current.get(task.id)) {
					patchTask(task.id, { status: "cancelled", error: null });
					return;
				}
				const message =
					error instanceof Error ? error.message : t("errors:unexpected_error");
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
		[markFolderForRefresh, markTaskFailed, patchTask, t],
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
			if (task.file) {
				params.set("declared_size", String(task.file.size));
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
				markFolderForRefresh(task);
			} catch (error) {
				if (controller.signal.aborted) {
					patchTask(task.id, { status: "cancelled", error: null });
					return;
				}
				const message =
					error instanceof Error ? error.message : t("errors:unexpected_error");
				markTaskFailed(task.id, message);
			} finally {
				directAbortRef.current.delete(task.id);
			}
		},
		[
			buildDirectUploadPath,
			markFolderForRefresh,
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
							adjustMultipartInFlight(task.id, 1);
							try {
								await uploadService.uploadChunk(uploadId, chunkNumber, blob);
							} finally {
								adjustMultipartInFlight(task.id, -1);
							}
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
				markFolderForRefresh(task);
			} catch (error) {
				if (abortFlagsRef.current.get(task.id)) {
					patchTask(task.id, { status: "cancelled", error: null });
					return;
				}
				const message =
					error instanceof Error ? error.message : t("errors:unexpected_error");
				markTaskFailed(task.id, message);
			} finally {
				abortFlagsRef.current.delete(task.id);
			}
		},
		[
			adjustMultipartInFlight,
			flushProgress,
			markFolderForRefresh,
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
				markFolderForRefresh(task);
			} catch (error) {
				const message =
					error instanceof Error ? error.message : t("errors:unexpected_error");
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
			flushProgress,
			markFolderForRefresh,
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
				alreadyCompleted.map((part) => part.part_number),
			);

			patchTask(task.id, {
				mode: "presigned_multipart" as UploadMode,
				status: "uploading",
				uploadId,
				totalChunks,
				completedChunks: completedSet.size,
				progress: Math.round((completedSet.size / totalChunks) * 90),
			});

			const queue = Array.from(
				{ length: totalChunks },
				(_, index) => index + 1,
			).filter((partNumber) => !completedSet.has(partNumber));

			const BATCH_SIZE = 50;
			let urlCache: Record<number, string> = {};

			const getPartUrl = async (partNumber: number): Promise<string> => {
				if (urlCache[partNumber]) return urlCache[partNumber];
				const index = queue.indexOf(partNumber);
				const batch =
					index >= 0 ? queue.slice(index, index + BATCH_SIZE) : [partNumber];
				const urls = await uploadService.presignParts(uploadId, batch);
				urlCache = { ...urlCache, ...urls };
				return urlCache[partNumber];
			};

			let completed = completedSet.size;

			const uploadOnePart = async () => {
				while (queue.length > 0) {
					if (abortFlagsRef.current.get(task.id)) return;
					const partNumber = queue.shift();
					if (partNumber === undefined) return;

					const start = (partNumber - 1) * chunkSize;
					const end = Math.min(start + chunkSize, file.size);
					const blob = file.slice(start, end);

					let lastError: Error | null = null;
					for (let attempt = 0; attempt < CHUNK_MAX_RETRIES; attempt++) {
						try {
							const url = await getPartUrl(partNumber);
							adjustMultipartInFlight(task.id, 1);
							const etag = await (async () => {
								try {
									return await uploadService.presignedUpload(url, blob);
								} finally {
									adjustMultipartInFlight(task.id, -1);
								}
							})();
							const part: CompletedPart = {
								part_number: partNumber,
								etag: etag.replace(/"/g, ""),
							};
							collectedParts.push(part);
							appendCompletedPart(uploadId, part);
							lastError = null;
							break;
						} catch (error) {
							lastError =
								error instanceof Error ? error : new Error(String(error));
							delete urlCache[partNumber];
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

				collectedParts.sort(
					(left, right) => left.part_number - right.part_number,
				);
				await completeWithRetry(uploadId, collectedParts);
				removeSession(uploadId);
				patchTask(task.id, {
					status: "completed",
					progress: 100,
					error: null,
				});
				markFolderForRefresh(task);
			} catch (error) {
				if (abortFlagsRef.current.get(task.id)) {
					patchTask(task.id, { status: "cancelled", error: null });
					return;
				}
				const message =
					error instanceof Error ? error.message : t("errors:unexpected_error");
				markTaskFailed(task.id, message);
			} finally {
				abortFlagsRef.current.delete(task.id);
			}
		},
		[
			adjustMultipartInFlight,
			flushProgress,
			markFolderForRefresh,
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

				const init = await uploadService.initUpload({
					filename: file.name,
					total_size: file.size,
					folder_id: task.baseFolderId,
					relative_path: task.relativePath ?? undefined,
				});
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
					error instanceof Error ? error.message : t("errors:unexpected_error");
				markTaskFailed(taskId, message);
			}
		},
		[
			markTaskFailed,
			patchTask,
			resumeCompletionTask,
			runChunkedUpload,
			runDirectUpload,
			runMultipartUpload,
			runPresignedUpload,
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
					} catch {}
				}
				patchTask(taskId, { status: "cancelled", error: null });
				return;
			}

			if (task.status === "pending_file") {
				if (task.uploadId) removeSession(task.uploadId);
				setTasks((prev) => prev.filter((item) => item.id !== taskId));
				return;
			}

			if (task.mode === "chunked" || task.mode === "presigned_multipart") {
				await cancelMultipartSession(task);
				patchTask(taskId, { status: "cancelled", error: null });
				return;
			}

			abortFlagsRef.current.set(taskId, true);
			if (task.uploadId) {
				try {
					await uploadService.cancelUpload(task.uploadId);
				} catch {}
				removeSession(task.uploadId);
			}
			patchTask(taskId, { status: "cancelled", error: null });
		},
		[cancelMultipartSession, patchTask],
	);

	const retryTask = useCallback(
		async (taskId: string) => {
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
				if (task.mode === "chunked" || task.mode === "presigned_multipart") {
					await cancelMultipartSession(task);
				} else {
					void uploadService.cancelUpload(task.uploadId).catch(() => undefined);
					removeSession(task.uploadId);
				}
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
		[cancelMultipartSession, patchTask, resumeCompletionTask, workspace],
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
		(event: ChangeEvent<HTMLInputElement>) => {
			addFiles(event.target.files);
			event.target.value = "";
		},
		[addFiles],
	);

	const handleFolderInputChange = useCallback(
		(event: ChangeEvent<HTMLInputElement>) => {
			const files = event.target.files;
			if (!files) return;
			addFilesWithPath(extractFilesFromInput(files));
			event.target.value = "";
		},
		[addFilesWithPath],
	);

	const handleDragEnter = useCallback((event: DragEvent<HTMLDivElement>) => {
		event.preventDefault();
		dragCounter.current += 1;
		if (event.dataTransfer.types.includes("Files")) setIsDragging(true);
	}, []);

	const handleDragLeave = useCallback((event: DragEvent<HTMLDivElement>) => {
		event.preventDefault();
		dragCounter.current -= 1;
		if (dragCounter.current === 0) setIsDragging(false);
	}, []);

	const handleDragOver = useCallback((event: DragEvent<HTMLDivElement>) => {
		event.preventDefault();
	}, []);

	const handleDrop = useCallback(
		async (event: DragEvent<HTMLDivElement>) => {
			event.preventDefault();
			dragCounter.current = 0;
			setIsDragging(false);
			if (
				event.dataTransfer.items?.length &&
				(event.dataTransfer.files.length === 0 ||
					hasDirectoryInDropItems(event.dataTransfer.items))
			) {
				const files = await extractFilesFromDrop(event.dataTransfer.items);
				addFilesWithPath(files);
				return;
			}
			addFiles(event.dataTransfer.files);
		},
		[addFiles, addFilesWithPath],
	);

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
		) {
			activeCount++;
		}
	}
	const totalCount = tasks.length;
	const overallProgress =
		totalCount === 0 ? 0 : Math.round(progressSum / totalCount);

	const uploadTasks: UploadTaskView[] = tasks.map((task) => {
		const isPendingFile = task.status === "pending_file";
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
						onClick: () => requestResumeFilePicker(task.id),
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
			title: task.filename,
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

	return {
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
		successCount,
		totalCount,
		uploadPanelOpen,
		uploadTasks,
	};
}
