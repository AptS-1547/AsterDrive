import {
	getProcessingProgress,
	S3_PROCESSING_PROGRESS,
} from "@/components/files/uploadResume";
import { getApiErrorMessage } from "@/hooks/useApiError";
import { api } from "@/services/http";
import type { InitUploadResponse } from "@/services/uploadService";
import {
	buildUploadPath,
	UploadRequestError,
	uploadService,
} from "@/services/uploadService";
import type { UploadTask } from "./uploadAreaManagerShared";
import { completeWithRetry } from "./uploadAreaManagerShared";
import type {
	UploadModeRunnerContext,
	UploadModeRunners,
} from "./uploadAreaUploadRunnerShared";

function buildDirectUploadPath(
	task: UploadTask,
	workspace: UploadModeRunnerContext["workspace"],
) {
	const params = new URLSearchParams();
	if (task.baseFolderId !== null) {
		params.set("folder_id", String(task.baseFolderId));
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
}

export function createSimpleUploadRunners({
	directAbortRef,
	flushProgress,
	markFolderForRefresh,
	markTaskFailed,
	patchTask,
	patchTaskThrottled,
	presignedXhrRef,
	workspace,
}: UploadModeRunnerContext): Pick<
	UploadModeRunners,
	"runDirectUpload" | "runPresignedUpload"
> {
	const runDirectUpload = async (task: UploadTask) => {
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
			await api.client.post(buildDirectUploadPath(task, workspace), formData, {
				headers: { "Content-Type": "multipart/form-data" },
				signal: controller.signal,
				timeout: 0,
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
			const message = getApiErrorMessage(error);
			markTaskFailed(task.id, message);
		} finally {
			directAbortRef.current.delete(task.id);
		}
	};

	const runPresignedUpload = async (
		task: UploadTask,
		init: InitUploadResponse,
	) => {
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
						progress: Math.round((loaded / total) * S3_PROCESSING_PROGRESS),
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
			if (error instanceof UploadRequestError && error.isAborted) {
				patchTask(task.id, { status: "cancelled", error: null });
				return;
			}
			const message = getApiErrorMessage(error);
			markTaskFailed(task.id, message);
		} finally {
			presignedXhrRef.current.delete(task.id);
		}
	};

	return {
		runDirectUpload,
		runPresignedUpload,
	};
}
