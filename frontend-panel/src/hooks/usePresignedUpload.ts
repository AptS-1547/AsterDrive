import { useCallback, useRef, useState } from "react";
import { uploadService } from "@/services/uploadService";

export interface PresignedUploadState {
	status: "idle" | "uploading" | "processing" | "completed" | "failed";
	uploadId: string | null;
	filename: string | null;
	progress: number;
	error: string | null;
}

const INITIAL_STATE: PresignedUploadState = {
	status: "idle",
	uploadId: null,
	filename: null,
	progress: 0,
	error: null,
};

export function usePresignedUpload(onComplete?: () => void) {
	const [state, setState] = useState<PresignedUploadState>(INITIAL_STATE);
	const abortRef = useRef(false);

	const startUpload = useCallback(
		async (file: File, uploadId: string, presignedUrl: string) => {
			abortRef.current = false;
			setState({
				status: "uploading",
				uploadId,
				filename: file.name,
				progress: 0,
				error: null,
			});

			try {
				// 1. PUT 直传 S3
				await uploadService.presignedUpload(
					presignedUrl,
					file,
					(loaded, total) => {
						if (abortRef.current) return;
						setState((s) => ({
							...s,
							progress: Math.round((loaded / total) * 90),
						}));
					},
				);

				if (abortRef.current) return;

				// 2. 通知服务端确认（hash + dedup + 建记录）
				setState((s) => ({ ...s, status: "processing", progress: 90 }));
				await uploadService.completeUpload(uploadId);

				setState((s) => ({
					...s,
					status: "completed",
					progress: 100,
				}));
				onComplete?.();
			} catch (err) {
				if (abortRef.current) return;
				const msg = err instanceof Error ? err.message : "upload failed";
				setState((s) => ({ ...s, status: "failed", error: msg }));
			}
		},
		[onComplete],
	);

	const cancelUpload = useCallback(async () => {
		abortRef.current = true;
		if (state.uploadId) {
			try {
				await uploadService.cancelUpload(state.uploadId);
			} catch {
				// ignore
			}
		}
		setState(INITIAL_STATE);
	}, [state.uploadId]);

	const reset = useCallback(() => {
		abortRef.current = false;
		setState(INITIAL_STATE);
	}, []);

	return { state, startUpload, cancelUpload, reset };
}
