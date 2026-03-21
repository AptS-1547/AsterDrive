import { FileIcon, Folder, RotateCcw, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { AppLayout } from "@/components/layout/AppLayout";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { trashService } from "@/services/trashService";
import type { FileInfo, FolderInfo } from "@/types/api";

function formatDate(dateStr: string | null | undefined): string {
	if (!dateStr) return "-";
	return new Date(dateStr).toLocaleDateString();
}

export default function TrashPage() {
	const [files, setFiles] = useState<FileInfo[]>([]);
	const [folders, setFolders] = useState<FolderInfo[]>([]);
	const [loading, setLoading] = useState(true);

	const load = useCallback(async () => {
		try {
			const data = await trashService.list();
			setFiles(data.files);
			setFolders(data.folders);
		} catch (err) {
			handleApiError(err);
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		load();
	}, [load]);

	const handleRestore = async (type: "file" | "folder", id: number) => {
		try {
			if (type === "file") await trashService.restoreFile(id);
			else await trashService.restoreFolder(id);
			toast.success("Restored");
			load();
		} catch (err) {
			handleApiError(err);
		}
	};

	const handlePurge = async (type: "file" | "folder", id: number) => {
		try {
			if (type === "file") await trashService.purgeFile(id);
			else await trashService.purgeFolder(id);
			toast.success("Permanently deleted");
			load();
		} catch (err) {
			handleApiError(err);
		}
	};

	const handlePurgeAll = async () => {
		try {
			await trashService.purgeAll();
			toast.success("Trash emptied");
			load();
		} catch (err) {
			handleApiError(err);
		}
	};

	const isEmpty = files.length === 0 && folders.length === 0;

	return (
		<AppLayout>
			<PageHeader
				title="Trash"
				actions={
					!isEmpty && (
						<Button variant="destructive" size="sm" onClick={handlePurgeAll}>
							<Trash2 className="h-4 w-4 mr-1" />
							Empty Trash
						</Button>
					)
				}
			/>
			<div className="flex-1 overflow-auto p-6">
				{loading ? (
					<div className="text-muted-foreground">Loading...</div>
				) : isEmpty ? (
					<div className="text-muted-foreground text-center py-12">
						Trash is empty
					</div>
				) : (
					<Table>
						<TableHeader>
							<TableRow>
								<TableHead className="w-[50%]">Name</TableHead>
								<TableHead>Deleted</TableHead>
								<TableHead className="w-[120px]">Actions</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{folders.map((f) => (
								<TableRow key={`folder-${f.id}`}>
									<TableCell className="flex items-center gap-2">
										<Folder className="h-4 w-4 text-blue-500" />
										{f.name}
									</TableCell>
									<TableCell className="text-muted-foreground">
										{formatDate(f.deleted_at)}
									</TableCell>
									<TableCell>
										<div className="flex gap-1">
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8"
												title="Restore"
												onClick={() => handleRestore("folder", f.id)}
											>
												<RotateCcw className="h-4 w-4" />
											</Button>
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8 text-destructive"
												title="Delete permanently"
												onClick={() => handlePurge("folder", f.id)}
											>
												<Trash2 className="h-4 w-4" />
											</Button>
										</div>
									</TableCell>
								</TableRow>
							))}
							{files.map((f) => (
								<TableRow key={`file-${f.id}`}>
									<TableCell className="flex items-center gap-2">
										<FileIcon className="h-4 w-4 text-muted-foreground" />
										{f.name}
									</TableCell>
									<TableCell className="text-muted-foreground">
										{formatDate(f.deleted_at)}
									</TableCell>
									<TableCell>
										<div className="flex gap-1">
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8"
												title="Restore"
												onClick={() => handleRestore("file", f.id)}
											>
												<RotateCcw className="h-4 w-4" />
											</Button>
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8 text-destructive"
												title="Delete permanently"
												onClick={() => handlePurge("file", f.id)}
											>
												<Trash2 className="h-4 w-4" />
											</Button>
										</div>
									</TableCell>
								</TableRow>
							))}
						</TableBody>
					</Table>
				)}
			</div>
		</AppLayout>
	);
}
