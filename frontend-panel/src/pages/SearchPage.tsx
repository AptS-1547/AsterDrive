import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AppLayout } from "@/components/layout/AppLayout";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { formatBytes, formatDate } from "@/lib/format";
import { searchService } from "@/services/searchService";
import type { FileInfo, FolderInfo } from "@/types/api";

export default function SearchPage() {
	const { t } = useTranslation();
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
			<div className="p-4 space-y-4">
				<div className="flex gap-2">
					<Input
						placeholder={t("search:placeholder")}
						value={query}
						onChange={(e) => setQuery(e.target.value)}
						onKeyDown={(e) => e.key === "Enter" && doSearch()}
						className="max-w-md"
					/>
					<Select
						value={searchType}
						onValueChange={(v) => {
							if (v) setSearchType(v);
						}}
					>
						<SelectTrigger className="w-[150px]">
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="all">{t("search:all")}</SelectItem>
							<SelectItem value="file">{t("search:files_only")}</SelectItem>
							<SelectItem value="folder">{t("search:folders_only")}</SelectItem>
						</SelectContent>
					</Select>
					<Button onClick={doSearch} disabled={loading || !query.trim()}>
						<Icon name="MagnifyingGlass" className="h-4 w-4 mr-1" />
						{t("search")}
					</Button>
				</div>

				{searched && !loading && (
					<div className="text-sm text-muted-foreground">
						{t("search:results", {
							files: totalFiles,
							folders: totalFolders,
						})}
					</div>
				)}

				{loading ? (
					<SkeletonTable columns={5} rows={8} />
				) : (
					<ScrollArea className="flex-1">
						{(folders.length > 0 || files.length > 0) && (
							<Table>
								<TableHeader>
									<TableRow>
										<TableHead className="w-8" />
										<TableHead>{t("name")}</TableHead>
										<TableHead>{t("type")}</TableHead>
										<TableHead>{t("size")}</TableHead>
										<TableHead>{t("created_at")}</TableHead>
									</TableRow>
								</TableHeader>
								<TableBody>
									{folders.map((f) => (
										<TableRow key={`folder-${f.id}`}>
											<TableCell>
												<Icon name="Folder" className="h-4 w-4 text-blue-500" />
											</TableCell>
											<TableCell className="font-medium">{f.name}</TableCell>
											<TableCell className="text-muted-foreground">
												{t("folder")}
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
												<Icon
													name="File"
													className="h-4 w-4 text-muted-foreground"
												/>
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
							<EmptyState
								icon={<Icon name="MagnifyingGlass" className="h-10 w-10" />}
								title={t("search:no_results")}
								description={t("search:no_results_desc")}
							/>
						)}
					</ScrollArea>
				)}
			</div>
		</AppLayout>
	);
}
