import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useParams } from "react-router-dom";
import { toast } from "sonner";
import { SkeletonCard } from "@/components/common/SkeletonCard";
import { FilePreview } from "@/components/files/FilePreview";
import { ReadOnlyFileCollection } from "@/components/files/ReadOnlyFileCollection";
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
import { ApiError } from "@/services/http";
import { shareService } from "@/services/shareService";
import type { FileInfo, FolderContents, SharePublicInfo } from "@/types/api";
import { ErrorCode } from "@/types/api";

interface ShareBreadcrumbItem {
	id: number | null;
	name: string;
}

export default function ShareViewPage() {
	const { t } = useTranslation();
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

	const SHARE_PAGE_SIZE = 100;
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
					setError(t("share_expired"));
				} else if (e.code === ErrorCode.ShareNotFound) {
					setError(t("share_not_found"));
				} else if (e.code === ErrorCode.ShareDownloadLimitReached) {
					setError(t("download_limit_reached"));
				} else {
					setError(e.message);
				}
			} else {
				setError(t("failed_to_load_share"));
			}
		} finally {
			setLoading(false);
		}
	}, [token, t]);

	useEffect(() => {
		loadInfo();
	}, [loadInfo]);

	const sharePageParams = {
		folder_limit: 1000,
		file_limit: SHARE_PAGE_SIZE,
	};

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
			// Current folder = last breadcrumb item
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

	// Infinite scroll for share page
	useEffect(() => {
		if (!hasMoreFiles || loadingMore) return;
		const el = sentinelRef.current;
		if (!el) return;
		const observer = new IntersectionObserver(
			(entries) => {
				if (entries[0].isIntersecting) void loadMoreShareFiles();
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
			toast.success(t("password_verified"));

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

	if (loading) {
		return (
			<div className="flex min-h-screen items-center justify-center bg-background px-6 py-8">
				<div className="w-full max-w-6xl">
					<SkeletonCard />
				</div>
			</div>
		);
	}

	if (error) {
		return (
			<div className="flex min-h-screen items-center justify-center bg-background px-6 py-8">
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
			</div>
		);
	}

	if (!info) return null;

	// Password gate
	if (needsPassword && !passwordVerified) {
		return (
			<div className="flex min-h-screen items-center justify-center bg-background px-6 py-8">
				<Card className="w-full max-w-md shadow-sm">
					<CardHeader className="text-center">
						<Icon
							name="Lock"
							className="mx-auto mb-2 h-10 w-10 text-muted-foreground"
						/>
						<CardTitle>{info.name}</CardTitle>
						<CardDescription>{t("password_protected")}</CardDescription>
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
			</div>
		);
	}

	// File share
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
			<div className="min-h-screen bg-background px-6 py-8">
				<div className="mx-auto flex max-w-4xl flex-col gap-6">
					<Card className="shadow-sm">
						<CardHeader>
							<div className="flex items-start gap-3">
								<div className="flex h-10 w-10 items-center justify-center rounded-xl bg-muted text-muted-foreground">
									<Icon name="File" className="h-5 w-5" />
								</div>
								<div className="min-w-0 flex-1">
									<CardTitle className="truncate">{info.name}</CardTitle>
									<CardDescription className="mt-1">
										{info.max_downloads > 0
											? t("n_of_m_downloads", {
													count: info.download_count,
													max: info.max_downloads,
												})
											: t("n_downloads", {
													count: info.download_count,
												})}
										{info.expires_at &&
											` · ${t("expires_date", {
												date: new Date(info.expires_at).toLocaleDateString(),
											})}`}
									</CardDescription>
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
						</CardHeader>
					</Card>
				</div>
				{previewFile && token && (
					<FilePreview
						file={previewFile}
						onClose={() => setPreviewFile(null)}
						downloadPath={shareService.downloadUrl(token)}
						editable={false}
					/>
				)}
			</div>
		);
	}

	// Folder share
	return (
		<div className="min-h-screen bg-background px-6 py-8">
			<div className="mx-auto flex max-w-6xl flex-col gap-6">
				<Card className="shadow-sm">
					<CardHeader>
						<div className="flex items-start gap-3">
							<div className="flex h-10 w-10 items-center justify-center rounded-xl bg-muted text-amber-500">
								<Icon name="Folder" className="h-5 w-5" />
							</div>
							<div className="min-w-0 flex-1">
								<CardTitle>{info.name}</CardTitle>
								<CardDescription className="mt-1">
									{t("shared_folder")}
									{info.expires_at &&
										` \u00b7 ${t("expires_date", { date: new Date(info.expires_at).toLocaleDateString() })}`}
								</CardDescription>
							</div>
						</div>
					</CardHeader>
				</Card>

				{breadcrumb.length > 1 && (
					<Breadcrumb>
						<BreadcrumbList>
							{breadcrumb.map((item, i) => (
								<Fragment key={item.id ?? "root"}>
									{i > 0 && <BreadcrumbSeparator />}
									<BreadcrumbItem>
										{i < breadcrumb.length - 1 ? (
											<BreadcrumbLink
												className="cursor-pointer"
												onClick={() => navigateToFolder(item.id, item.name)}
											>
												{item.name}
											</BreadcrumbLink>
										) : (
											<BreadcrumbPage>{item.name}</BreadcrumbPage>
										)}
									</BreadcrumbItem>
								</Fragment>
							))}
						</BreadcrumbList>
					</Breadcrumb>
				)}

				{navigating ? (
					<SkeletonCard />
				) : folderContents ? (
					<>
						<ReadOnlyFileCollection
							folders={folderContents.folders}
							files={folderContents.files}
							viewMode={viewMode}
							onViewModeChange={setViewMode}
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
					<Card className="shadow-sm">
						<CardContent className="py-8 text-sm text-muted-foreground">
							{t("loading_contents")}
						</CardContent>
					</Card>
				)}
			</div>
			{previewFile && token && (
				<FilePreview
					file={previewFile}
					onClose={() => setPreviewFile(null)}
					downloadPath={shareService.downloadFolderFileUrl(
						token,
						previewFile.id,
					)}
					editable={false}
				/>
			)}
		</div>
	);
}
