import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { fileService } from "@/services/fileService";
import type { FileInfo } from "@/types/api";

interface FilePreviewProps {
	file: FileInfo;
	onClose: () => void;
}

export function FilePreview({ file, onClose }: FilePreviewProps) {
	const isImage =
		file.mime_type.startsWith("image/") && file.mime_type !== "image/svg+xml";
	const isText =
		file.mime_type.startsWith("text/") ||
		file.mime_type === "application/json" ||
		file.mime_type === "application/xml";
	const isPdf = file.mime_type === "application/pdf";
	const isVideo = file.mime_type.startsWith("video/");
	const isAudio = file.mime_type.startsWith("audio/");

	const downloadUrl = fileService.downloadUrl(file.id);

	return (
		<div
			className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center"
			onClick={onClose}
		>
			<div
				className="relative max-w-[90vw] max-h-[90vh] flex flex-col"
				onClick={(e) => e.stopPropagation()}
			>
				<div className="flex items-center justify-between p-3 bg-background/90 rounded-t-lg">
					<span className="text-sm font-medium truncate">{file.name}</span>
					<Button variant="ghost" size="icon" onClick={onClose}>
						<X className="h-4 w-4" />
					</Button>
				</div>
				<div className="flex-1 overflow-auto bg-background/50 rounded-b-lg p-2">
					{isImage && (
						<img
							src={downloadUrl}
							alt={file.name}
							className="max-w-full max-h-[80vh] object-contain mx-auto"
						/>
					)}
					{isVideo && (
						<video
							src={downloadUrl}
							controls
							className="max-w-full max-h-[80vh] mx-auto"
						/>
					)}
					{isAudio && (
						<audio src={downloadUrl} controls className="w-full mt-4" />
					)}
					{isPdf && (
						<iframe
							src={downloadUrl}
							title={file.name}
							className="w-[80vw] h-[80vh]"
						/>
					)}
					{isText && <TextPreview url={downloadUrl} />}
					{!isImage && !isVideo && !isAudio && !isPdf && !isText && (
						<div className="text-center text-muted-foreground py-12">
							Preview not available for this file type
						</div>
					)}
				</div>
			</div>
		</div>
	);
}

function TextPreview({ url }: { url: string }) {
	const [content, setContent] = useState<string | null>(null);
	const [error, setError] = useState(false);

	useEffect(() => {
		fetch(url, { credentials: "include" })
			.then((r) => r.text())
			.then(setContent)
			.catch(() => setError(true));
	}, [url]);

	if (error) return <div className="text-destructive p-4">Failed to load</div>;
	if (content === null)
		return <div className="text-muted-foreground p-4">Loading...</div>;

	return (
		<pre className="text-sm font-mono whitespace-pre-wrap p-4 max-h-[80vh] overflow-auto">
			{content}
		</pre>
	);
}
