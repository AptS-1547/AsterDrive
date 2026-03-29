import {
	Fragment,
	lazy,
	Suspense,
	useCallback,
	useEffect,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { useParams } from "react-router-dom";
import { toast } from "sonner";
import { SkeletonCard } from "@/components/common/SkeletonCard";
import { ToolbarBar } from "@/components/common/ToolbarBar";
import { UserAvatarImage } from "@/components/common/UserAvatarImage";
import { ViewToggle } from "@/components/common/ViewToggle";
import { FileThumbnail } from "@/components/files/FileThumbnail";
import { ReadOnlyFileCollection } from "@/components/files/ReadOnlyFileCollection";
import { ShareTopBar } from "@/components/layout/ShareTopBar";
import {
	Breadcrumb,
	BreadcrumbItem,
	BreadcrumbLink,
	BreadcrumbList,
	BreadcrumbPage,
	BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Button } from "@/components/ui/button";
import {
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { handleApiError } from "@/hooks/useApiError";
import { FOLDER_LIMIT, PAGE_SECTION_PADDING_CLASS } from "@/lib/constants";
import { formatDateShort } from "@/lib/format";
import { ApiError } from "@/services/http";
import { shareService } from "@/services/shareService";
import type { FileInfo, FolderContents, SharePublicInfo } from "@/types/api";
import { ErrorCode } from "@/types/api";

interface ShareBreadcrumbItem {
	id: number | null;
	name: string;
}

const SHARE_PAGE_SIZE = 100;
const sharePageParams = {
	folder_limit: FOLDER_LIMIT,
	file_limit: SHARE_PAGE_SIZE,
};

function ShareOwnerBanner({
	owner,
	text,
}: {
	owner: SharePublicInfo["shared_by"];
	text: string;
}) {
	return (
		<div className="flex max-w-full items-center gap-3 rounded-xl border border-border/70 bg-muted/20 px-3 py-3 text-sm text-muted-foreground">
			<UserAvatarImage
				avatar={owner.avatar}
				name={owner.name}
				size="sm"
				className="rounded-xl"
			/>
			<span className="min-w-0 truncate">{text}</span>
		</div>
	);
}

const FilePreview = lazy(async () => {
	const module = await import("@/components/files/FilePreview");
	return { default: module.FilePreview };
});

export default function ShareViewPage() {
	const { t } = useTranslation(["core", "share"]);
	const { token } = useParams<{ token: string }>();
	const [info, setInfo] = useState<SharePublicInfo | null>(null);
	const [needsPassword, setNeedsPassword] = useState(false);
	const [passwordVerified, setPasswordVerified] = useState(false);
	const [password, setPassword] = useState("");
	const [loading, setLoading] = useState(true);
	const [error, setError] = useState<string | null>(null);
	const [folderContents, setFolderContents] = useState<FolderContents | null>(
		null,
	);
	const [viewMode, setViewMode] = useState<"grid" | "list">("grid");
	const [previewFile, setPreviewFile] = useState<FileInfo | null>(null);
	const [breadcrumb, setBreadcrumb] = useState<ShareBreadcrumbItem[]>([]);
	const [navigating, setNavigating] = useState(false);
	const [loadingMore, setLoadingMore] = useState(false);
	const sentinelRef = useRef<HTMLDivElement | null>(null);

	const hasMoreFiles = folderContents?.next_file_cursor != null;

	const loadInfo = useCallback(async () => {
		if (!token) return;
		try {
			setLoading(true);
			const data = await shareService.getInfo(token);
			setInfo(data);
			setNeedsPassword(data.has_password);

			if (data.share_type === "folder" && !data.has_password) {
				const contents = await shareService.listContent(token, sharePageParams);
				setFolderContents(contents);
				setBreadcrumb([{ id: null, name: data.name }]);
			}
		} catch (e) {
			if (e instanceof ApiError) {
				if (e.code === ErrorCode.ShareExpired) {
					setError(t("share:share_expired"));
				} else if (e.code === ErrorCode.ShareNotFound) {
					setError(t("share:share_not_found"));
				} else if (e.code === ErrorCode.ShareDownloadLimitReached) {
					setError(t("share:download_limit_reached"));
				} else {
					setError(e.message);
				}
			} else {
				setError(t("share:failed_to_load_share"));
			}
		} finally {
			setLoading(false);
		}
	}, [token, t]);

	useEffect(() => {
		void loadInfo().catch(() => {});
	}, [loadInfo]);

	const navigateToFolder = useCallback(
		async (folderId: number | null, folderName?: string) => {
			if (!token) return;
			setNavigating(true);
			try {
				const contents =
					folderId === null
						? await shareService.listContent(token, sharePageParams)
						: await shareService.listSubfolderContent(
								token,
								folderId,
								sharePageParams,
							);
				setFolderContents(contents);

				setBreadcrumb((prev) => {
					if (folderId === null) {
						return [prev[0]];
					}
					const existingIndex = prev.findIndex((b) => b.id === folderId);
					if (existingIndex >= 0) {
						return prev.slice(0, existingIndex + 1);
					}
					return [...prev, { id: folderId, name: folderName ?? "" }];
				});
			} catch (e) {
				handleApiError(e);
			} finally {
				setNavigating(false);
			}
		},
		[token],
	);

	const loadMoreShareFiles = useCallback(async () => {
		if (
			!token ||
			!folderContents ||
			loadingMore ||
			!folderContents.next_file_cursor
		)
			return;
		setLoadingMore(true);
		try {
			const currentId = breadcrumb[breadcrumb.length - 1]?.id ?? null;
			const cursor = folderContents.next_file_cursor;
			const contents =
				currentId === null
					? await shareService.listContent(token, {
							folder_limit: 0,
							file_limit: SHARE_PAGE_SIZE,
							file_after_value: cursor.value,
							file_after_id: cursor.id,
						})
					: await shareService.listSubfolderContent(token, currentId, {
							folder_limit: 0,
							file_limit: SHARE_PAGE_SIZE,
							file_after_value: cursor.value,
							file_after_id: cursor.id,
						});
			setFolderContents((prev) =>
				prev
					? {
							...prev,
							files: [...prev.files, ...contents.files],
							next_file_cursor: contents.next_file_cursor,
						}
					: prev,
			);
		} catch (e) {
			handleApiError(e);
		} finally {
			setLoadingMore(false);
		}
	}, [token, folderContents, loadingMore, breadcrumb]);

	useEffect(() => {
		if (!hasMoreFiles || loadingMore) return;
		const el = sentinelRef.current;
		if (!el) return;
		const observer = new IntersectionObserver(
			(entries) => {
				if (entries[0].isIntersecting)
					void loadMoreShareFiles().catch(() => {});
			},
			{ rootMargin: "200px" },
		);
		observer.observe(el);
		return () => observer.disconnect();
	}, [hasMoreFiles, loadingMore, loadMoreShareFiles]);

	const handleVerifyPassword = async (e: React.FormEvent) => {
		e.preventDefault();
		if (!token) return;
		try {
			await shareService.verifyPassword(token, password);
			setPasswordVerified(true);
			setNeedsPassword(false);
			toast.success(t("share:password_verified"));

			if (info?.share_type === "folder") {
				const contents = await shareService.listContent(token, sharePageParams);
				setFolderContents(contents);
				setBreadcrumb([{ id: null, name: info.name }]);
			}
		} catch (e) {
			handleApiError(e);
		}
	};

	const handleDownload = () => {
		if (!token) return;
		const url = shareService.downloadUrl(token);
		window.open(url, "_blank");
	};

	const handleFolderFileDownload = (file: FileInfo) => {
		if (!token) return;
		const url = shareService.downloadFolderFileUrl(token, file.id);
		window.open(url, "_blank");
	};

	// Breadcrumb renderer for folder share
	const breadcrumbElement =
		breadcrumb.length > 1 ? (
			<Breadcrumb>
				<BreadcrumbList className="gap-2">
					{breadcrumb.map((item, i) => (
						<Fragment key={item.id ?? "root"}>
							{i > 0 && (
								<BreadcrumbSeparator className="mx-0.5 text-muted-foreground/45" />
							)}
							<BreadcrumbItem>
								{i < breadcrumb.length - 1 ? (
									<BreadcrumbLink
										className="cursor-pointer rounded-md px-1.5 py-0.5 text-muted-foreground"
										onClick={() => navigateToFolder(item.id, item.name)}
									>
										{item.name}
									</BreadcrumbLink>
								) : (
									<BreadcrumbPage className="text-base font-semibold text-foreground">
										{item.name}
									</BreadcrumbPage>
								)}
							</BreadcrumbItem>
						</Fragment>
					))}
				</BreadcrumbList>
			</Breadcrumb>
		) : null;

	const previewElement =
		previewFile && token ? (
			<Suspense fallback={null}>
				<FilePreview
					file={previewFile}
					onClose={() => setPreviewFile(null)}
					downloadPath={
						info?.share_type === "file"
							? shareService.downloadPath(token)
							: shareService.downloadFolderPath(token, previewFile.id)
					}
					editable={false}
				/>
			</Suspense>
		) : null;

	// ── Centered states (loading, error, password) ──
	if (loading) {
		return (
			<div className="h-screen flex flex-col">
				<ShareTopBar />
				<main className="flex min-h-0 flex-1 items-center justify-center p-6">
					<div className="w-full max-w-6xl">
						<SkeletonCard />
					</div>
				</main>
			</div>
		);
	}

	if (error) {
		return (
			<div className="h-screen flex flex-col">
				<ShareTopBar />
				<main className="flex min-h-0 flex-1 items-center justify-center p-6">
					<Card className="w-full max-w-md">
						<CardHeader className="text-center">
							<Icon
								name="Warning"
								className="mx-auto mb-2 h-10 w-10 text-destructive"
							/>
							<CardTitle>{t("unavailable")}</CardTitle>
							<CardDescription>{error}</CardDescription>
						</CardHeader>
					</Card>
				</main>
			</div>
		);
	}

	if (!info) return null;
	if (!token) return null;

	const shareOwnerText = t("share:shared_with_you", {
		name: info.shared_by.name,
		resource: info.name,
	});

	if (needsPassword && !passwordVerified) {
		return (
			<div className="h-screen flex flex-col">
				<ShareTopBar />
				<main className="flex min-h-0 flex-1 items-center justify-center p-6">
					<Card className="w-full max-w-md shadow-sm">
						<CardHeader className="text-center">
							<Icon
								name="Lock"
								className="mx-auto mb-2 h-10 w-10 text-muted-foreground"
							/>
							<CardTitle>{info.name}</CardTitle>
							<CardDescription>{t("share:password_protected")}</CardDescription>
						</CardHeader>
						<CardContent>
							<form onSubmit={handleVerifyPassword} className="space-y-4">
								<Input
									type="password"
									placeholder={t("auth:password")}
									value={password}
									onChange={(e) => setPassword(e.target.value)}
									autoFocus
								/>
								<Button type="submit" className="w-full">
									{t("verify")}
								</Button>
							</form>
						</CardContent>
					</Card>
				</main>
			</div>
		);
	}

	// ── File share ──
	if (info.share_type === "file") {
		const singleShareFile =
			info.mime_type && typeof info.size === "number"
				? ({
						id: -1,
						name: info.name,
						mime_type: info.mime_type,
						size: info.size,
						folder_id: null,
						blob_id: 0,
						user_id: 0,
						created_at: new Date().toISOString(),
						updated_at: new Date().toISOString(),
						deleted_at: null,
						is_locked: false,
					} satisfies FileInfo)
				: null;

		return (
			<div className="h-screen flex flex-col">
				<ShareTopBar />
				<main className="flex min-h-0 flex-1 items-center justify-center p-6">
					<Card className="w-full max-w-4xl shadow-sm">
						<CardHeader>
							<div className="space-y-4">
								<div className="flex flex-col gap-4 sm:flex-row sm:items-start">
									<div className="flex min-w-0 flex-1 items-start gap-3">
										<div className="flex h-14 w-14 shrink-0 items-center justify-center overflow-hidden rounded-xl bg-muted/40">
											{singleShareFile ? (
												<div className="flex h-full w-full items-center justify-center">
													<FileThumbnail
														file={singleShareFile}
														size="lg"
														thumbnailPath={shareService.thumbnailPath(token)}
													/>
												</div>
											) : (
												<Icon
													name="File"
													className="h-6 w-6 text-muted-foreground"
												/>
											)}
										</div>
										<div className="min-w-0 flex-1">
											<CardTitle className="truncate">{info.name}</CardTitle>
											<CardDescription className="mt-1">
												{info.max_downloads > 0
													? t("share:n_of_m_downloads", {
															count: info.download_count,
															max: info.max_downloads,
														})
													: t("share:n_downloads", {
															count: info.download_count,
														})}
												{info.expires_at &&
													` · ${t("share:expires_date", {
														date: formatDateShort(info.expires_at),
													})}`}
											</CardDescription>
										</div>
									</div>
									<div className="flex shrink-0 items-center gap-2">
										{singleShareFile ? (
											<Button
												variant="outline"
												onClick={() => setPreviewFile(singleShareFile)}
											>
												<Icon name="Eye" className="mr-2 h-4 w-4" />
												{t("files:preview")}
											</Button>
										) : null}
										<Button onClick={handleDownload}>
											<Icon name="Download" className="mr-2 h-4 w-4" />
											{t("files:download")}
										</Button>
									</div>
								</div>
								<ShareOwnerBanner
									owner={info.shared_by}
									text={shareOwnerText}
								/>
							</div>
						</CardHeader>
					</Card>
				</main>
				{previewElement}
			</div>
		);
	}

	// ── Folder share ──
	return (
		<div className="h-screen flex flex-col">
			<ShareTopBar />
			<ToolbarBar
				left={
					<>
						<Icon name="Folder" className="h-4 w-4 shrink-0 text-amber-500" />
						<div className="min-w-0 flex-1">
							{breadcrumbElement ?? (
								<span className="text-base font-semibold text-foreground">
									{info.name}
								</span>
							)}
						</div>
					</>
				}
				right={<ViewToggle value={viewMode} onChange={setViewMode} />}
			/>
			<div className="border-b border-border/70 bg-background/80">
				<div className={`${PAGE_SECTION_PADDING_CLASS} py-3`}>
					<ShareOwnerBanner owner={info.shared_by} text={shareOwnerText} />
				</div>
			</div>
			<div className="min-h-0 flex-1 overflow-auto">
				{navigating ? (
					<div className="p-6">
						<SkeletonCard />
					</div>
				) : folderContents ? (
					<>
						<ReadOnlyFileCollection
							folders={folderContents.folders}
							files={folderContents.files}
							viewMode={viewMode}
							onFileClick={setPreviewFile}
							onFileDownload={handleFolderFileDownload}
							onFolderClick={(folder) =>
								navigateToFolder(folder.id, folder.name)
							}
							getThumbnailPath={(file) =>
								`/s/${token}/files/${file.id}/thumbnail`
							}
							emptyTitle={t("empty_folder")}
							emptyDescription={t("folder_empty_desc")}
						/>
						{hasMoreFiles && (
							<div ref={sentinelRef} className="flex justify-center py-4">
								{loadingMore && (
									<div className="h-5 w-5 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-muted-foreground" />
								)}
							</div>
						)}
					</>
				) : (
					<div className="p-6 text-sm text-muted-foreground">
						{t("loading_contents")}
					</div>
				)}
			</div>
			{previewElement}
		</div>
	);
}
