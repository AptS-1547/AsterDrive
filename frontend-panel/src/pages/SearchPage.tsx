import { FileIcon, Folder, Search } from "lucide-react";
import { useCallback, useState } from "react";
import { AppLayout } from "@/components/layout/AppLayout";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { searchService } from "@/services/searchService";
import type { FileInfo, FolderInfo } from "@/types/api";

function formatDate(dateStr: string): string {
	return new Date(dateStr).toLocaleString();
}

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`;
}

export default function SearchPage() {
	const [query, setQuery] = useState("");
	const [searchType, setSearchType] = useState("all");
	const [files, setFiles] = useState<Array<FileInfo & { size: number }>>([]);
	const [folders, setFolders] = useState<FolderInfo[]>([]);
	const [totalFiles, setTotalFiles] = useState(0);
	const [totalFolders, setTotalFolders] = useState(0);
	const [loading, setLoading] = useState(false);
	const [searched, setSearched] = useState(false);

	const doSearch = useCallback(async () => {
		if (!query.trim()) return;
		setLoading(true);
		try {
			const results = await searchService.search({
				q: query.trim(),
				type: searchType === "all" ? undefined : searchType,
				limit: 50,
			});
			setFiles(results.files);
			setFolders(results.folders);
			setTotalFiles(results.total_files);
			setTotalFolders(results.total_folders);
			setSearched(true);
		} catch (err) {
			handleApiError(err);
		} finally {
			setLoading(false);
		}
	}, [query, searchType]);

	return (
		<AppLayout>
			<PageHeader title="Search" />
			<div className="p-4 space-y-4">
				<div className="flex gap-2">
					<Input
						placeholder="Search files and folders..."
						value={query}
						onChange={(e) => setQuery(e.target.value)}
						onKeyDown={(e) => e.key === "Enter" && doSearch()}
						className="max-w-md"
					/>
					<select
						value={searchType}
						onChange={(e) => setSearchType(e.target.value)}
						className="border rounded-md px-3 py-2 text-sm bg-background"
					>
						<option value="all">All</option>
						<option value="file">Files only</option>
						<option value="folder">Folders only</option>
					</select>
					<Button onClick={doSearch} disabled={loading || !query.trim()}>
						<Search className="h-4 w-4 mr-1" />
						Search
					</Button>
				</div>

				{searched && (
					<div className="text-sm text-muted-foreground">
						Found {totalFiles} file(s) and {totalFolders} folder(s)
					</div>
				)}

				<ScrollArea className="flex-1">
					{(folders.length > 0 || files.length > 0) && (
						<Table>
							<TableHeader>
								<TableRow>
									<TableHead className="w-8" />
									<TableHead>Name</TableHead>
									<TableHead>Type</TableHead>
									<TableHead>Size</TableHead>
									<TableHead>Created</TableHead>
								</TableRow>
							</TableHeader>
							<TableBody>
								{folders.map((f) => (
									<TableRow key={`folder-${f.id}`}>
										<TableCell>
											<Folder className="h-4 w-4 text-blue-500" />
										</TableCell>
										<TableCell className="font-medium">{f.name}</TableCell>
										<TableCell className="text-muted-foreground">
											Folder
										</TableCell>
										<TableCell>---</TableCell>
										<TableCell className="text-muted-foreground">
											{formatDate(f.created_at)}
										</TableCell>
									</TableRow>
								))}
								{files.map((f) => (
									<TableRow key={`file-${f.id}`}>
										<TableCell>
											<FileIcon className="h-4 w-4 text-muted-foreground" />
										</TableCell>
										<TableCell className="font-medium">{f.name}</TableCell>
										<TableCell className="text-muted-foreground">
											{f.mime_type}
										</TableCell>
										<TableCell className="text-muted-foreground">
											{formatBytes(f.size)}
										</TableCell>
										<TableCell className="text-muted-foreground">
											{formatDate(f.created_at)}
										</TableCell>
									</TableRow>
								))}
							</TableBody>
						</Table>
					)}
					{searched && folders.length === 0 && files.length === 0 && (
						<div className="text-center py-12 text-muted-foreground">
							No results found
						</div>
					)}
				</ScrollArea>
			</div>
		</AppLayout>
	);
}
