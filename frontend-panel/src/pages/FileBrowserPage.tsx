import {
	ChevronDown,
	ChevronUp,
	FolderPlus,
	Layers,
	LogOut,
	Shield,
} from "lucide-react";
import type { FormEvent } from "react";
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { toast } from "sonner";
import { FileList } from "@/components/files/FileList";
import { UploadArea } from "@/components/files/UploadArea";
import { AppLayout } from "@/components/layout/AppLayout";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
	DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { handleApiError } from "@/hooks/useApiError";
import { batchService } from "@/services/batchService";
import { useAuthStore } from "@/stores/authStore";
import { useFileStore } from "@/stores/fileStore";

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`;
}

function StorageIndicator({
	user,
}: {
	user: { storage_used: number; storage_quota: number };
}) {
	const used = user.storage_used ?? 0;
	const quota = user.storage_quota ?? 0;
	const pct = quota > 0 ? Math.min((used / quota) * 100, 100) : 0;

	return (
		<div className="flex items-center gap-2 text-xs text-muted-foreground">
			<span>
				{formatBytes(used)}
				{quota > 0 ? ` / ${formatBytes(quota)}` : ""}
			</span>
			{quota > 0 && <Progress value={pct} className="h-1.5 w-16" />}
		</div>
	);
}

function parseIds(s: string): number[] {
	return s
		.split(",")
		.map((x) => Number(x.trim()))
		.filter((x) => !Number.isNaN(x) && x > 0);
}

export default function FileBrowserPage() {
	const navigateTo = useFileStore((s) => s.navigateTo);
	const createFolder = useFileStore((s) => s.createFolder);
	const refresh = useFileStore((s) => s.refresh);
	const logout = useAuthStore((s) => s.logout);
	const user = useAuthStore((s) => s.user);
	const [folderName, setFolderName] = useState("");
	const [dialogOpen, setDialogOpen] = useState(false);
	const [batchOpen, setBatchOpen] = useState(false);
	const [batchFileIds, setBatchFileIds] = useState("");
	const [batchFolderIds, setBatchFolderIds] = useState("");

	const handleBatchDelete = async () => {
		try {
			const result = await batchService.batchDelete(
				parseIds(batchFileIds),
				parseIds(batchFolderIds),
			);
			toast.success(
				`Deleted: ${result.succeeded} succeeded, ${result.failed} failed`,
			);
			for (const e of result.errors) {
				toast.error(`${e.entity_type} #${e.entity_id}: ${e.error}`);
			}
			setBatchFileIds("");
			setBatchFolderIds("");
			await refresh();
		} catch (err) {
			handleApiError(err);
		}
	};

	const handleBatchMove = async () => {
		try {
			const result = await batchService.batchMove(
				parseIds(batchFileIds),
				parseIds(batchFolderIds),
				null,
			);
			toast.success(
				`Moved to root: ${result.succeeded} succeeded, ${result.failed} failed`,
			);
			for (const e of result.errors) {
				toast.error(`${e.entity_type} #${e.entity_id}: ${e.error}`);
			}
			setBatchFileIds("");
			setBatchFolderIds("");
			await refresh();
		} catch (err) {
			handleApiError(err);
		}
	};

	const handleBatchCopy = async () => {
		try {
			const result = await batchService.batchCopy(
				parseIds(batchFileIds),
				parseIds(batchFolderIds),
				null,
			);
			toast.success(
				`Copied to root: ${result.succeeded} succeeded, ${result.failed} failed`,
			);
			for (const e of result.errors) {
				toast.error(`${e.entity_type} #${e.entity_id}: ${e.error}`);
			}
			setBatchFileIds("");
			setBatchFolderIds("");
			await refresh();
		} catch (err) {
			handleApiError(err);
		}
	};

	useEffect(() => {
		navigateTo(null).catch(handleApiError);
	}, [navigateTo]);

	const handleCreateFolder = async (e: FormEvent) => {
		e.preventDefault();
		if (!folderName.trim()) return;
		try {
			await createFolder(folderName.trim());
			toast.success("Folder created");
			setFolderName("");
			setDialogOpen(false);
		} catch (error) {
			handleApiError(error);
		}
	};

	return (
		<AppLayout>
			<PageHeader
				actions={
					<>
						{user && <StorageIndicator user={user} />}
						<Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
							<DialogTrigger render={<Button variant="outline" size="sm" />}>
								<FolderPlus className="h-4 w-4 mr-1" />
								New Folder
							</DialogTrigger>
							<DialogContent>
								<DialogHeader>
									<DialogTitle>Create Folder</DialogTitle>
								</DialogHeader>
								<form onSubmit={handleCreateFolder} className="space-y-4">
									<Input
										placeholder="Folder name"
										value={folderName}
										onChange={(e) => setFolderName(e.target.value)}
										autoFocus
									/>
									<Button type="submit" className="w-full">
										Create
									</Button>
								</form>
							</DialogContent>
						</Dialog>
						{user?.role === "admin" && (
							<Link to="/admin">
								<Button variant="ghost" size="sm">
									<Shield className="h-4 w-4" />
								</Button>
							</Link>
						)}
						<Button variant="ghost" size="sm" onClick={logout}>
							<LogOut className="h-4 w-4" />
						</Button>
					</>
				}
			/>
			{/* Batch Operations PoC */}
			<div className="px-4 py-2 border-b">
				<button
					type="button"
					onClick={() => setBatchOpen(!batchOpen)}
					className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
				>
					<Layers className="h-4 w-4" />
					Batch Operations
					{batchOpen ? (
						<ChevronUp className="h-3 w-3" />
					) : (
						<ChevronDown className="h-3 w-3" />
					)}
				</button>
				{batchOpen && (
					<div className="mt-2 flex flex-wrap gap-2 items-end">
						<div>
							<label
								htmlFor="batch-file-ids"
								className="text-xs text-muted-foreground"
							>
								File IDs
							</label>
							<Input
								id="batch-file-ids"
								value={batchFileIds}
								onChange={(e) => setBatchFileIds(e.target.value)}
								placeholder="1, 2, 3"
								className="w-40 h-8"
							/>
						</div>
						<div>
							<label
								htmlFor="batch-folder-ids"
								className="text-xs text-muted-foreground"
							>
								Folder IDs
							</label>
							<Input
								id="batch-folder-ids"
								value={batchFolderIds}
								onChange={(e) => setBatchFolderIds(e.target.value)}
								placeholder="1, 2"
								className="w-40 h-8"
							/>
						</div>
						<Button size="sm" variant="destructive" onClick={handleBatchDelete}>
							Delete
						</Button>
						<Button size="sm" variant="outline" onClick={handleBatchMove}>
							Move to Root
						</Button>
						<Button size="sm" variant="outline" onClick={handleBatchCopy}>
							Copy to Root
						</Button>
					</div>
				)}
			</div>
			<UploadArea>
				<ScrollArea className="flex-1">
					<FileList />
				</ScrollArea>
			</UploadArea>
		</AppLayout>
	);
}
