import { Uppy } from "@uppy/core";
import XHRUpload from "@uppy/xhr-upload";
import { RefreshCw, Upload, X } from "lucide-react";
import type { DragEvent, ReactNode } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { config } from "@/config/app";
import { useChunkedUpload } from "@/hooks/useChunkedUpload";
import { usePresignedUpload } from "@/hooks/usePresignedUpload";
import { cn } from "@/lib/utils";
import { uploadService } from "@/services/uploadService";
import { useFileStore } from "@/stores/fileStore";

interface UploadAreaProps {
	children: ReactNode;
}

export function UploadArea({ children }: UploadAreaProps) {
	const refresh = useFileStore((s) => s.refresh);
	const currentFolderId = useFileStore((s) => s.currentFolderId);
	const currentFolderIdRef = useRef(currentFolderId);
	const [isDragging, setIsDragging] = useState(false);
	const dragCounter = useRef(0);

	// Uppy progress for small files
	const [uppyProgress, setUppyProgress] = useState<{
		filename: string;
		percent: number;
	} | null>(null);

	// Chunked upload for large files
	const {
		state: chunkedState,
		startUpload,
		resumeUpload,
		cancelUpload,
		reset,
	} = useChunkedUpload(() => {
		toast.success("Upload complete (chunked)");
		refresh();
	});

	// Presigned upload for S3 direct
	const {
		state: presignedState,
		startUpload: presignedStartUpload,
		cancelUpload: presignedCancel,
		reset: presignedReset,
	} = usePresignedUpload(() => {
		toast.success("Upload complete (S3 direct)");
		refresh();
	});

	// Resume file ref — user must re-select file to resume
	const resumeInputRef = useRef<HTMLInputElement | null>(null);

	useEffect(() => {
		currentFolderIdRef.current = currentFolderId;
	}, [currentFolderId]);

	const [uppy] = useState(() => {
		const instance = new Uppy({
			restrictions: { maxNumberOfFiles: 10 },
			autoProceed: true,
		});
		instance.use(XHRUpload, {
			endpoint: `${config.apiBaseUrl}/files/upload`,
			fieldName: "file",
			withCredentials: true,
		});
		instance.on("progress", (progress) => {
			// progress is 0-100 for all files combined
			if (progress > 0 && progress < 100) {
				setUppyProgress((p) => ({
					filename: p?.filename ?? "Uploading...",
					percent: progress,
				}));
			}
		});
		instance.on("upload", () => {
			const files = instance.getFiles();
			setUppyProgress({
				filename: files[0]?.name ?? "Uploading...",
				percent: 0,
			});
		});
		instance.on("complete", (result) => {
			setUppyProgress(null);
			const count = result.successful?.length ?? 0;
			if (count > 0) {
				toast.success(`Uploaded ${count} file(s)`);
				refresh();
			}
			instance.cancelAll();
		});
		instance.on("error", (error) => {
			setUppyProgress(null);
			toast.error(`Upload failed: ${error.message}`);
		});
		return instance;
	});

	// biome-ignore lint/correctness/useExhaustiveDependencies: currentFolderId triggers re-sync intentionally
	useEffect(() => {
		const folderId = currentFolderIdRef.current;
		const base = `${config.apiBaseUrl}/files/upload`;
		const endpoint = folderId !== null ? `${base}?folder_id=${folderId}` : base;
		const xhrPlugin = uppy.getPlugin("XHRUpload");
		if (xhrPlugin) {
			xhrPlugin.setOptions({ endpoint });
		}
	}, [currentFolderId, uppy]);

	useEffect(() => {
		return () => uppy.destroy();
	}, [uppy]);

	const addFileViaUppy = (file: File) => {
		try {
			uppy.addFile({ name: file.name, type: file.type, data: file });
		} catch (err) {
			if (err instanceof Error && !err.message.includes("already been added")) {
				toast.error(err.message);
			}
		}
	};

	const addFiles = async (files: FileList | null) => {
		if (!files || files.length === 0) return;
		for (const file of files) {
			try {
				// 向服务端协商上传模式
				const resp = await uploadService.initUpload({
					filename: file.name,
					total_size: file.size,
					folder_id: currentFolderIdRef.current,
				});
				if (resp.mode === "presigned") {
					presignedStartUpload(
						file,
						resp.upload_id as string,
						resp.presigned_url as string,
					);
				} else if (resp.mode === "chunked") {
					startUpload(file, currentFolderIdRef.current, resp);
				} else {
					addFileViaUppy(file);
				}
			} catch {
				// init 失败（配额/大小限制等），fallback 到直传让后端再报具体错误
				addFileViaUppy(file);
			}
		}
	};

	const handleResume = useCallback(() => {
		resumeInputRef.current?.click();
	}, []);

	const handleResumeFileSelected = useCallback(
		(e: React.ChangeEvent<HTMLInputElement>) => {
			const file = e.target.files?.[0];
			if (file) resumeUpload(file);
			e.target.value = "";
		},
		[resumeUpload],
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
	const handleDrop = (e: DragEvent<HTMLDivElement>) => {
		e.preventDefault();
		dragCounter.current = 0;
		setIsDragging(false);
		addFiles(e.dataTransfer.files);
	};

	const showChunkedProgress =
		chunkedState.status !== "idle" && chunkedState.status !== "completed";
	const showResumePrompt =
		chunkedState.canResume && chunkedState.status === "idle";
	const showPresignedProgress =
		presignedState.status !== "idle" && presignedState.status !== "completed";

	return (
		// biome-ignore lint/a11y/noStaticElementInteractions: drop zone
		<div
			className="relative flex-1 flex flex-col overflow-hidden"
			onDragEnter={handleDragEnter}
			onDragLeave={handleDragLeave}
			onDragOver={handleDragOver}
			onDrop={handleDrop}
		>
			{children}

			{/* Hidden input for resume file selection */}
			<input
				ref={resumeInputRef}
				type="file"
				className="hidden"
				onChange={handleResumeFileSelected}
			/>

			{/* Resume prompt — shown on page load if there's a saved session */}
			{showResumePrompt && (
				<div className="absolute bottom-4 right-4 z-40 w-80 bg-card border rounded-lg shadow-lg p-4 space-y-2">
					<p className="text-sm font-medium">
						Incomplete upload: {chunkedState.filename}
					</p>
					<p className="text-xs text-muted-foreground">
						Select the same file to resume
					</p>
					<div className="flex gap-2">
						<Button size="sm" className="flex-1" onClick={handleResume}>
							<RefreshCw className="h-3.5 w-3.5 mr-1" />
							Resume
						</Button>
						<Button size="sm" variant="outline" onClick={reset}>
							Dismiss
						</Button>
					</div>
				</div>
			)}

			{/* Chunked upload progress */}
			{showChunkedProgress && (
				<div className="absolute bottom-4 right-4 z-40 w-80 bg-card border rounded-lg shadow-lg p-4 space-y-2">
					<div className="flex items-center justify-between">
						<div className="text-sm font-medium truncate flex-1 mr-2">
							{chunkedState.filename}
						</div>
						<Button
							variant="ghost"
							size="icon"
							className="h-6 w-6 shrink-0"
							onClick={cancelUpload}
						>
							<X className="h-3.5 w-3.5" />
						</Button>
					</div>
					<Progress value={chunkedState.progress} className="h-2" />
					<div className="flex justify-between text-xs text-muted-foreground">
						<span>
							{chunkedState.status === "assembling"
								? "Assembling..."
								: chunkedState.status === "failed"
									? (chunkedState.error ?? "Failed")
									: `Chunk ${chunkedState.completedChunks}/${chunkedState.totalChunks}`}
						</span>
						<span className="text-muted-foreground/60">
							{chunkedState.progress}% chunked
						</span>
					</div>
					{chunkedState.status === "failed" && (
						<div className="flex gap-2">
							<Button
								size="sm"
								variant="outline"
								className="flex-1"
								onClick={handleResume}
							>
								<RefreshCw className="h-3.5 w-3.5 mr-1" />
								Retry
							</Button>
							<Button size="sm" variant="outline" onClick={reset}>
								Dismiss
							</Button>
						</div>
					)}
				</div>
			)}

			{/* Presigned S3 direct upload progress */}
			{showPresignedProgress && (
				<div className="absolute bottom-4 right-4 z-40 w-80 bg-card border rounded-lg shadow-lg p-4 space-y-2">
					<div className="flex items-center justify-between">
						<div className="text-sm font-medium truncate flex-1 mr-2">
							{presignedState.filename}
						</div>
						<Button
							variant="ghost"
							size="icon"
							className="h-6 w-6 shrink-0"
							onClick={presignedCancel}
						>
							<X className="h-3.5 w-3.5" />
						</Button>
					</div>
					<Progress value={presignedState.progress} className="h-2" />
					<div className="flex justify-between text-xs text-muted-foreground">
						<span>
							{presignedState.status === "processing"
								? "Processing..."
								: presignedState.status === "failed"
									? (presignedState.error ?? "Failed")
									: "Uploading to S3..."}
						</span>
						<span className="text-muted-foreground/60">
							{presignedState.progress}% S3 direct
						</span>
					</div>
					{presignedState.status === "failed" && (
						<Button
							size="sm"
							variant="outline"
							className="w-full"
							onClick={presignedReset}
						>
							Dismiss
						</Button>
					)}
				</div>
			)}

			{/* Uppy small-file upload progress */}
			{uppyProgress && (
				<div className="absolute bottom-4 right-4 z-40 w-72 bg-card border rounded-lg shadow-lg p-3 space-y-1.5">
					<div className="text-sm font-medium truncate">
						{uppyProgress.filename}
					</div>
					<Progress value={uppyProgress.percent} className="h-1.5" />
					<div className="text-xs text-muted-foreground text-right">
						{uppyProgress.percent}% direct
					</div>
				</div>
			)}

			{/* Drag overlay */}
			{isDragging && (
				<div
					className={cn(
						"absolute inset-0 z-50 flex flex-col items-center justify-center",
						"bg-background/80 backdrop-blur-sm border-2 border-dashed border-primary rounded-lg",
					)}
				>
					<Upload className="h-10 w-10 text-primary mb-3" />
					<p className="text-lg font-medium text-primary">
						Drop files to upload
					</p>
					<p className="text-sm text-muted-foreground mt-1">
						Files will be uploaded to the current folder
					</p>
				</div>
			)}
		</div>
	);
}
