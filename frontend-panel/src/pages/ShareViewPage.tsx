import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useParams } from "react-router-dom";
import { toast } from "sonner";
import { SkeletonCard } from "@/components/common/SkeletonCard";
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
import type { FolderContents, SharePublicInfo } from "@/types/api";
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

	if (loading) {
		return (
			<div className="min-h-screen flex items-center justify-center bg-background">
				<SkeletonCard />
			</div>
		);
	}

	if (error) {
		return (
			<div className="min-h-screen flex items-center justify-center bg-background">
				<Card className="w-full max-w-sm">
					<CardHeader className="text-center">
						<Icon
							name="Warning"
							className="h-10 w-10 text-destructive mx-auto mb-2"
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
			<div className="min-h-screen flex items-center justify-center bg-background">
				<Card className="w-full max-w-sm">
					<CardHeader className="text-center">
						<Icon
							name="Lock"
							className="h-10 w-10 text-muted-foreground mx-auto mb-2"
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
		return (
			<div className="min-h-screen flex items-center justify-center bg-background">
				<Card className="w-full max-w-sm">
					<CardHeader className="text-center">
						<Icon
							name="File"
							className="h-10 w-10 text-muted-foreground mx-auto mb-2"
						/>
						<CardTitle>{info.name}</CardTitle>
						<CardDescription>
							{info.max_downloads > 0
								? t("n_of_m_downloads", {
										count: info.download_count,
										max: info.max_downloads,
									})
								: t("n_downloads", {
										count: info.download_count,
									})}
							{info.expires_at &&
								` \u00b7 ${t("expires_date", { date: new Date(info.expires_at).toLocaleDateString() })}`}
						</CardDescription>
					</CardHeader>
					<CardContent>
						<Button className="w-full" onClick={handleDownload}>
							<Icon name="Download" className="h-4 w-4 mr-2" />
							{t("files:download")}
						</Button>
					</CardContent>
				</Card>
			</div>
		);
	}

	// Folder share
	return (
		<div className="min-h-screen flex items-center justify-center bg-background">
			<Card className="w-full max-w-lg">
				<CardHeader>
					<div className="flex items-center gap-2">
						<Icon name="Folder" className="h-5 w-5 text-blue-500" />
						<CardTitle>{info.name}</CardTitle>
					</div>
					<CardDescription>
						{t("shared_folder")}
						{info.expires_at &&
							` \u00b7 ${t("expires_date", { date: new Date(info.expires_at).toLocaleDateString() })}`}
					</CardDescription>
				</CardHeader>
				<CardContent>
					{folderContents ? (
						<div className="space-y-1">
							{folderContents.folders.map((f) => (
								<div
									key={`folder-${f.id}`}
									className="flex items-center gap-2 px-3 py-2 rounded-md text-sm text-muted-foreground"
								>
									<Icon name="Folder" className="h-4 w-4 text-blue-500" />
									{f.name}
								</div>
							))}
							{folderContents.files.map((f) => (
								<div
									key={`file-${f.id}`}
									className="flex items-center gap-2 px-3 py-2 rounded-md text-sm"
								>
									<Icon name="File" className="h-4 w-4 text-muted-foreground" />
									{f.name}
								</div>
							))}
							{folderContents.folders.length === 0 &&
								folderContents.files.length === 0 && (
									<p className="text-sm text-muted-foreground text-center py-4">
										{t("empty_folder")}
									</p>
								)}
						</div>
					) : (
						<p className="text-sm text-muted-foreground">
							{t("loading_contents")}
						</p>
					)}
				</CardContent>
			</Card>
		</div>
	);
}
