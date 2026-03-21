import { History, RotateCcw, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
	DialogTrigger,
} from "@/components/ui/dialog";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { fileService } from "@/services/fileService";
import type { FileVersion } from "@/services/fileService";

function formatSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
	if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
	return `${(bytes / 1073741824).toFixed(1)} GB`;
}

interface VersionHistoryDialogProps {
	fileId: number;
	fileName: string;
	onRestored?: () => void;
}

export function VersionHistoryDialog({
	fileId,
	fileName,
	onRestored,
}: VersionHistoryDialogProps) {
	const [open, setOpen] = useState(false);
	const [versions, setVersions] = useState<FileVersion[]>([]);
	const [loading, setLoading] = useState(false);

	const load = useCallback(async () => {
		try {
			setLoading(true);
			const data = await fileService.listVersions(fileId);
			setVersions(data);
		} catch (e) {
			handleApiError(e);
		} finally {
			setLoading(false);
		}
	}, [fileId]);

	useEffect(() => {
		if (open) load();
	}, [open, load]);

	const handleRestore = async (versionId: number) => {
		try {
			await fileService.restoreVersion(fileId, versionId);
			toast.success("Version restored");
			load();
			onRestored?.();
		} catch (e) {
			handleApiError(e);
		}
	};

	const handleDelete = async (versionId: number) => {
		try {
			await fileService.deleteVersion(fileId, versionId);
			toast.success("Version deleted");
			setVersions((prev) => prev.filter((v) => v.id !== versionId));
		} catch (e) {
			handleApiError(e);
		}
	};

	return (
		<Dialog open={open} onOpenChange={setOpen}>
			<DialogTrigger
				render={<Button variant="ghost" size="icon" className="h-8 w-8" />}
			>
				<History className="h-4 w-4" />
			</DialogTrigger>
			<DialogContent className="max-w-lg">
				<DialogHeader>
					<DialogTitle>Version History - {fileName}</DialogTitle>
				</DialogHeader>
				{loading ? (
					<p className="text-muted-foreground text-sm py-4 text-center">
						Loading...
					</p>
				) : versions.length === 0 ? (
					<p className="text-muted-foreground text-sm py-4 text-center">
						No previous versions
					</p>
				) : (
					<Table>
						<TableHeader>
							<TableRow>
								<TableHead>Version</TableHead>
								<TableHead>Size</TableHead>
								<TableHead>Date</TableHead>
								<TableHead className="w-20">Actions</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{versions.map((v) => (
								<TableRow key={v.id}>
									<TableCell className="font-mono text-sm">
										v{v.version}
									</TableCell>
									<TableCell className="text-sm">
										{formatSize(v.size)}
									</TableCell>
									<TableCell className="text-muted-foreground text-xs">
										{new Date(v.created_at).toLocaleString()}
									</TableCell>
									<TableCell>
										<div className="flex gap-1">
											<Button
												variant="ghost"
												size="icon"
												className="h-7 w-7"
												title="Restore"
												onClick={() => handleRestore(v.id)}
											>
												<RotateCcw className="h-3.5 w-3.5" />
											</Button>
											<Button
												variant="ghost"
												size="icon"
												className="h-7 w-7 text-destructive"
												title="Delete"
												onClick={() => handleDelete(v.id)}
											>
												<Trash2 className="h-3.5 w-3.5" />
											</Button>
										</div>
									</TableCell>
								</TableRow>
							))}
						</TableBody>
					</Table>
				)}
			</DialogContent>
		</Dialog>
	);
}
