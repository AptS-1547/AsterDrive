import {
	type KeyboardEvent,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { FileThumbnail } from "@/components/files/FileThumbnail";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { handleApiError } from "@/hooks/useApiError";
import { formatBytes, formatDate } from "@/lib/format";
import { cn } from "@/lib/utils";
import { workspaceFolderPath } from "@/lib/workspace";
import { fileService } from "@/services/fileService";
import { isRequestCanceled } from "@/services/http";
import { searchService } from "@/services/searchService";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import type { FileListItem, FolderListItem, SearchResults } from "@/types/api";

const SEARCH_DEBOUNCE_MS = 180;
const SEARCH_RESULT_LIMIT = 10;
const EMPTY_RESULTS: SearchResults = {
	files: [],
	folders: [],
	total_files: 0,
	total_folders: 0,
};

type SearchFilter = "all" | "file" | "folder";

type SearchEntry =
	| { key: string; kind: "folder"; item: FolderListItem }
	| { key: string; kind: "file"; item: FileListItem };

interface GlobalSearchDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
}

interface SearchPreviewLocationState {
	searchPreviewFile?: FileListItem;
}

const SEARCH_FILTER_OPTIONS: Array<{
	value: SearchFilter;
	labelKey: string;
}> = [
	{ value: "all", labelKey: "all" },
	{ value: "file", labelKey: "files_only" },
	{ value: "folder", labelKey: "folders_only" },
];

function getSearchOffset(results: SearchResults) {
	return Math.max(results.files.length, results.folders.length);
}

function hasMoreSearchResults(results: SearchResults) {
	return (
		results.files.length < results.total_files ||
		results.folders.length < results.total_folders
	);
}

function SearchResultRow({
	active,
	entry,
	index,
	opening,
	onHover,
	onSelect,
}: {
	active: boolean;
	entry: SearchEntry;
	index: number;
	opening: boolean;
	onHover: (index: number) => void;
	onSelect: (entry: SearchEntry) => void;
}) {
	const { t, i18n } = useTranslation(["files", "search"]);

	return (
		<button
			type="button"
			data-search-result-index={index}
			className={cn(
				"flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left transition-colors",
				active
					? "bg-primary/10 text-foreground ring-1 ring-primary/20"
					: "hover:bg-muted/60",
			)}
			onMouseEnter={() => onHover(index)}
			onClick={() => onSelect(entry)}
		>
			<div
				className={cn(
					"flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-xl border border-border/60 bg-background/80",
					active && "border-primary/25 bg-primary/10 text-primary",
				)}
			>
				{entry.kind === "folder" ? (
					<Icon name="FolderOpen" className="h-4 w-4 text-amber-500" />
				) : (
					<FileThumbnail file={entry.item} size="md" />
				)}
			</div>
			<div className="min-w-0 flex-1 space-y-1">
				<div className="flex min-w-0 items-center gap-2">
					<span className="truncate text-sm font-medium">
						{entry.item.name}
					</span>
					{entry.item.is_locked ? (
						<Icon
							name="Lock"
							className="h-3.5 w-3.5 shrink-0 text-muted-foreground"
						/>
					) : null}
				</div>
				<div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
					<span>
						{entry.kind === "folder"
							? t("files:folders_section")
							: t("files:files_section")}
					</span>
					{entry.kind === "file" ? (
						<>
							<span className="h-1 w-1 rounded-full bg-border" />
							<span>{formatBytes(entry.item.size)}</span>
						</>
					) : null}
					<span className="h-1 w-1 rounded-full bg-border" />
					<span>{formatDate(entry.item.updated_at, i18n)}</span>
				</div>
			</div>
			<div className="flex shrink-0 items-center gap-2 text-muted-foreground">
				{opening ? (
					<Icon name="Spinner" className="h-4 w-4 animate-spin" />
				) : (
					<Icon name="ArrowSquareOut" className="h-4 w-4" />
				)}
			</div>
		</button>
	);
}

export function GlobalSearchDialog({
	open,
	onOpenChange,
}: GlobalSearchDialogProps) {
	const { t } = useTranslation(["core", "files", "search"]);
	const navigate = useNavigate();
	const workspace = useWorkspaceStore((state) => state.workspace);
	const inputRef = useRef<HTMLInputElement | null>(null);
	const resultListRef = useRef<HTMLDivElement | null>(null);
	const loadMoreRef = useRef<HTMLDivElement | null>(null);
	const controllerRef = useRef<AbortController | null>(null);
	const requestIdRef = useRef(0);
	const [query, setQuery] = useState("");
	const [filter, setFilter] = useState<SearchFilter>("all");
	const [results, setResults] = useState<SearchResults>(EMPTY_RESULTS);
	const [loading, setLoading] = useState(false);
	const [loadingMore, setLoadingMore] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const [activeIndex, setActiveIndex] = useState(0);
	const [openingKey, setOpeningKey] = useState<string | null>(null);

	const trimmedQuery = query.trim();

	const resultEntries = useMemo<SearchEntry[]>(
		() => [
			...results.folders.map((item) => ({
				key: `folder-${item.id}`,
				kind: "folder" as const,
				item,
			})),
			...results.files.map((item) => ({
				key: `file-${item.id}`,
				kind: "file" as const,
				item,
			})),
		],
		[results],
	);
	const activeEntry = resultEntries[activeIndex] ?? null;
	const canLoadMore = hasMoreSearchResults(results);

	useEffect(() => {
		if (!open) {
			controllerRef.current?.abort();
			controllerRef.current = null;
			requestIdRef.current += 1;
			setQuery("");
			setFilter("all");
			setResults(EMPTY_RESULTS);
			setLoading(false);
			setLoadingMore(false);
			setError(null);
			setActiveIndex(0);
			setOpeningKey(null);
			return;
		}

		const timer = window.setTimeout(() => {
			inputRef.current?.focus();
			inputRef.current?.select();
		}, 0);
		return () => window.clearTimeout(timer);
	}, [open]);

	useEffect(() => {
		if (!open) {
			return;
		}

		if (!trimmedQuery) {
			controllerRef.current?.abort();
			controllerRef.current = null;
			requestIdRef.current += 1;
			setResults(EMPTY_RESULTS);
			setLoading(false);
			setLoadingMore(false);
			setError(null);
			setActiveIndex(0);
			return;
		}

		const requestId = requestIdRef.current + 1;
		requestIdRef.current = requestId;
		controllerRef.current?.abort();
		const controller = new AbortController();
		controllerRef.current = controller;
		setLoading(true);
		setLoadingMore(false);
		setError(null);

		const timer = window.setTimeout(() => {
			void searchService
				.search(
					{
						q: trimmedQuery,
						type: filter,
						limit: SEARCH_RESULT_LIMIT,
					},
					{ signal: controller.signal },
				)
				.then((nextResults) => {
					if (requestIdRef.current !== requestId) {
						return;
					}
					setResults(nextResults);
					setLoading(false);
					setLoadingMore(false);
					setActiveIndex(0);
				})
				.catch((searchError) => {
					if (
						requestIdRef.current !== requestId ||
						isRequestCanceled(searchError)
					) {
						return;
					}
					setResults(EMPTY_RESULTS);
					setLoading(false);
					setLoadingMore(false);
					setError(
						searchError instanceof Error
							? searchError.message
							: t("search:search_error"),
					);
				});
		}, SEARCH_DEBOUNCE_MS);

		return () => {
			window.clearTimeout(timer);
			controller.abort();
			if (
				requestIdRef.current === requestId &&
				controllerRef.current === controller
			) {
				controllerRef.current = null;
			}
		};
	}, [filter, open, t, trimmedQuery]);

	useEffect(() => {
		if (
			!open ||
			!trimmedQuery ||
			!canLoadMore ||
			loading ||
			loadingMore ||
			typeof window === "undefined" ||
			typeof window.IntersectionObserver === "undefined"
		) {
			return;
		}

		const root = resultListRef.current;
		const sentinel = loadMoreRef.current;
		if (!root || !sentinel) {
			return;
		}

		const observer = new window.IntersectionObserver(
			(entries) => {
				if (!entries.some((entry) => entry.isIntersecting)) {
					return;
				}

				const requestId = requestIdRef.current + 1;
				requestIdRef.current = requestId;
				controllerRef.current?.abort();
				const controller = new AbortController();
				controllerRef.current = controller;
				setLoadingMore(true);

				void searchService
					.search(
						{
							q: trimmedQuery,
							type: filter,
							limit: SEARCH_RESULT_LIMIT,
							offset: getSearchOffset(results),
						},
						{ signal: controller.signal },
					)
					.then((nextResults) => {
						if (requestIdRef.current !== requestId) {
							return;
						}

						setResults((current) => ({
							files: [...current.files, ...nextResults.files],
							folders: [...current.folders, ...nextResults.folders],
							total_files: nextResults.total_files,
							total_folders: nextResults.total_folders,
						}));
						setLoadingMore(false);
					})
					.catch((searchError) => {
						if (
							requestIdRef.current !== requestId ||
							isRequestCanceled(searchError)
						) {
							return;
						}

						setLoadingMore(false);
					})
					.finally(() => {
						if (
							requestIdRef.current === requestId &&
							controllerRef.current === controller
						) {
							controllerRef.current = null;
						}
					});
			},
			{ root, rootMargin: "160px" },
		);

		observer.observe(sentinel);
		return () => observer.disconnect();
	}, [canLoadMore, filter, loading, loadingMore, open, results, trimmedQuery]);

	useEffect(() => {
		if (activeIndex < resultEntries.length) {
			return;
		}
		setActiveIndex(Math.max(0, resultEntries.length - 1));
	}, [activeIndex, resultEntries.length]);

	useEffect(() => {
		if (!open || resultEntries.length === 0) {
			return;
		}
		const activeRow = resultListRef.current?.querySelector<HTMLElement>(
			`[data-search-result-index="${activeIndex}"]`,
		);
		activeRow?.scrollIntoView({ block: "nearest" });
	}, [activeIndex, open, resultEntries.length]);

	const handleSelect = async (entry: SearchEntry) => {
		if (openingKey !== null) {
			return;
		}

		if (entry.kind === "folder") {
			onOpenChange(false);
			navigate(workspaceFolderPath(workspace, entry.item.id, entry.item.name), {
				viewTransition: true,
			});
			return;
		}

		setOpeningKey(entry.key);
		try {
			const fileInfo = await fileService.getFile(entry.item.id);
			const state: SearchPreviewLocationState = {
				searchPreviewFile: entry.item,
			};
			onOpenChange(false);
			navigate(workspaceFolderPath(workspace, fileInfo.folder_id ?? null), {
				state,
				viewTransition: true,
			});
		} catch (selectError) {
			handleApiError(selectError);
		} finally {
			setOpeningKey(null);
		}
	};

	const handleInputKeyDown = async (event: KeyboardEvent<HTMLInputElement>) => {
		if (event.key === "ArrowDown") {
			if (resultEntries.length === 0) {
				return;
			}
			event.preventDefault();
			setActiveIndex((current) => (current + 1) % resultEntries.length);
			return;
		}

		if (event.key === "ArrowUp") {
			if (resultEntries.length === 0) {
				return;
			}
			event.preventDefault();
			setActiveIndex((current) =>
				current === 0 ? resultEntries.length - 1 : current - 1,
			);
			return;
		}

		if (event.key === "Enter" && activeEntry) {
			event.preventDefault();
			await handleSelect(activeEntry);
			return;
		}

		if (event.key === "Escape") {
			event.preventDefault();
			onOpenChange(false);
		}
	};

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent
				showCloseButton={false}
				className="max-w-[min(760px,calc(100vw-1rem))] gap-0 overflow-hidden p-0 sm:max-w-[min(760px,calc(100vw-2rem))]"
			>
				<DialogHeader className="sr-only">
					<DialogTitle>{t("search:dialog_title")}</DialogTitle>
				</DialogHeader>
				<div className="border-b bg-background/95 px-4 py-3">
					<div className="flex items-center gap-3">
						<div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-2xl bg-primary/8 text-primary">
							<Icon name="MagnifyingGlass" className="h-5 w-5" />
						</div>
						<div className="min-w-0 flex-1">
							<Input
								ref={inputRef}
								value={query}
								onChange={(event) => setQuery(event.target.value)}
								onKeyDown={(event) => {
									void handleInputKeyDown(event);
								}}
								placeholder={t("search:placeholder")}
								autoComplete="off"
								spellCheck={false}
								className="h-11 border-none bg-transparent px-0 text-base shadow-none focus-visible:border-none focus-visible:ring-0"
							/>
						</div>
						<Button
							type="button"
							variant="ghost"
							size="sm"
							onClick={() => onOpenChange(false)}
							className="shrink-0"
						>
							<Icon name="X" className="h-4 w-4" />
						</Button>
					</div>
					<div className="mt-3 flex flex-wrap items-center gap-2">
						{SEARCH_FILTER_OPTIONS.map((option) => (
							<Button
								key={option.value}
								type="button"
								variant={filter === option.value ? "secondary" : "ghost"}
								size="sm"
								onClick={() => setFilter(option.value)}
								className="rounded-full"
							>
								{t(`search:${option.labelKey}`)}
							</Button>
						))}
					</div>
				</div>
				<div className="flex max-h-[min(68vh,560px)] min-h-80 flex-col">
					<div className="border-b bg-muted/20 px-4 py-2 text-xs text-muted-foreground">
						{trimmedQuery
							? t("search:results", {
									files: results.total_files,
									folders: results.total_folders,
								})
							: t("search:start_typing")}
					</div>
					<div ref={resultListRef} className="flex-1 overflow-y-auto px-2 py-2">
						{!trimmedQuery ? (
							<div className="grid gap-3 px-2 py-3 sm:grid-cols-2">
								<div className="rounded-2xl border border-border/60 bg-muted/25 p-4">
									<div className="mb-3 flex h-10 w-10 items-center justify-center rounded-xl bg-background text-primary ring-1 ring-border/60">
										<Icon name="MagnifyingGlass" className="h-4 w-4" />
									</div>
									<div className="space-y-1">
										<p className="text-sm font-medium">
											{t("search:start_typing")}
										</p>
										<p className="text-sm text-muted-foreground">
											{t("search:start_typing_desc")}
										</p>
									</div>
								</div>
								<div className="rounded-2xl border border-border/60 bg-muted/25 p-4">
									<div className="mb-3 flex h-10 w-10 items-center justify-center rounded-xl bg-background text-primary ring-1 ring-border/60">
										<Icon name="ArrowDown" className="h-4 w-4" />
									</div>
									<div className="space-y-1">
										<p className="text-sm font-medium">
											{t("search:navigate_results")}
										</p>
										<p className="text-sm text-muted-foreground">
											{t("search:navigate_results_desc")}
										</p>
									</div>
								</div>
							</div>
						) : loading && resultEntries.length === 0 ? (
							<div className="flex h-full min-h-56 flex-col items-center justify-center gap-3 text-sm text-muted-foreground">
								<Icon name="Spinner" className="h-5 w-5 animate-spin" />
								<span>{t("search:searching")}</span>
							</div>
						) : error ? (
							<div className="flex h-full min-h-56 flex-col items-center justify-center gap-3 px-4 text-center">
								<div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-destructive/10 text-destructive">
									<Icon name="Warning" className="h-5 w-5" />
								</div>
								<div className="space-y-1">
									<p className="text-sm font-medium">
										{t("search:search_error")}
									</p>
									<p className="text-sm text-muted-foreground">{error}</p>
								</div>
							</div>
						) : resultEntries.length === 0 ? (
							<div className="flex h-full min-h-56 flex-col items-center justify-center gap-3 px-4 text-center">
								<div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-muted text-muted-foreground">
									<Icon name="MagnifyingGlass" className="h-5 w-5" />
								</div>
								<div className="space-y-1">
									<p className="text-sm font-medium">
										{t("search:no_results")}
									</p>
									<p className="text-sm text-muted-foreground">
										{t("search:no_results_desc")}
									</p>
								</div>
							</div>
						) : (
							<div className="space-y-4 px-1 py-1">
								{results.folders.length > 0 ? (
									<div className="space-y-1">
										<div className="px-2 text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
											{t("files:folders_section")}
										</div>
										<div className="space-y-1">
											{results.folders.map((item, index) => (
												<SearchResultRow
													key={`folder-${item.id}`}
													active={activeIndex === index}
													entry={{
														key: `folder-${item.id}`,
														kind: "folder",
														item,
													}}
													index={index}
													opening={openingKey === `folder-${item.id}`}
													onHover={setActiveIndex}
													onSelect={(entry) => {
														void handleSelect(entry);
													}}
												/>
											))}
										</div>
									</div>
								) : null}
								{results.files.length > 0 ? (
									<div className="space-y-1">
										<div className="px-2 text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
											{t("files:files_section")}
										</div>
										<div className="space-y-1">
											{results.files.map((item, index) => {
												const absoluteIndex = results.folders.length + index;
												return (
													<SearchResultRow
														key={`file-${item.id}`}
														active={activeIndex === absoluteIndex}
														entry={{
															key: `file-${item.id}`,
															kind: "file",
															item,
														}}
														index={absoluteIndex}
														opening={openingKey === `file-${item.id}`}
														onHover={setActiveIndex}
														onSelect={(entry) => {
															void handleSelect(entry);
														}}
													/>
												);
											})}
										</div>
									</div>
								) : null}
								{canLoadMore ? (
									<div
										ref={loadMoreRef}
										data-search-load-more="true"
										className="flex justify-center py-3"
									>
										{loadingMore ? (
											<div className="h-5 w-5 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-muted-foreground" />
										) : (
											<div className="h-5 w-5" />
										)}
									</div>
								) : null}
							</div>
						)}
					</div>
					<div className="flex items-center justify-between gap-3 border-t bg-muted/20 px-4 py-2 text-xs text-muted-foreground">
						<span>{t("search:open_selected_desc")}</span>
						<div className="flex items-center gap-1.5">
							<kbd className="rounded-md border border-border/70 bg-background px-2 py-1 font-sans">
								Enter
							</kbd>
							<kbd className="rounded-md border border-border/70 bg-background px-2 py-1 font-sans">
								Esc
							</kbd>
						</div>
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}
