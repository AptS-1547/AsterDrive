import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useParams } from "react-router-dom";
import { toast } from "sonner";
import { SkeletonCard } from "@/components/common/SkeletonCard";
import { FilePreview } from "@/components/files/FilePreview";
import { ReadOnlyFileCollection } from "@/components/files/ReadOnlyFileCollection";
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

	const loadInfo = useCallback(async () => {
		if (!token) return;
		try {
			setLoading(true);
			const data = await shareService.getInfo(token);
			setInfo(data);
			setNeedsPassword(data.has_password);

			if (data.share_type === "folder" && !data.has_password) {
				const contents = await shareService.listContent(token);
				setFolderContents(contents);
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

	const handleVerifyPassword = async (e: React.FormEvent) => {
		e.preventDefault();
		if (!token) return;
		try {
			await shareService.verifyPassword(token, password);
			setPasswordVerified(true);
			setNeedsPassword(false);
			toast.success(t("password_verified"));

			if (info?.share_type === "folder") {
				const contents = await shareService.listContent(token);
				setFolderContents(contents);
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

				{folderContents ? (
					<ReadOnlyFileCollection
						folders={folderContents.folders}
						files={folderContents.files}
						viewMode={viewMode}
						onViewModeChange={setViewMode}
						onFileClick={setPreviewFile}
						onFileDownload={handleFolderFileDownload}
						emptyTitle={t("empty_folder")}
						emptyDescription={t("folder_empty_desc")}
					/>
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
