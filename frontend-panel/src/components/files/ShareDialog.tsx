import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { handleApiError } from "@/hooks/useApiError";
import { shareService } from "@/services/shareService";

interface ShareDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	fileId?: number;
	folderId?: number;
	name: string;
}

export function ShareDialog({
	open,
	onOpenChange,
	fileId,
	folderId,
	name,
}: ShareDialogProps) {
	const { t } = useTranslation(["common", "files"]);
	const [password, setPassword] = useState("");
	const [expiry, setExpiry] = useState("never");
	const [maxDownloads, setMaxDownloads] = useState("");
	const [shareUrl, setShareUrl] = useState<string | null>(null);
	const [copied, setCopied] = useState(false);
	const [loading, setLoading] = useState(false);

	const handleCreate = async (e: React.FormEvent) => {
		e.preventDefault();
		setLoading(true);
		try {
			const expiresAt = computeExpiry(expiry);
			const downloads = maxDownloads ? Number.parseInt(maxDownloads, 10) : 0;

			const share = await shareService.create({
				file_id: fileId,
				folder_id: folderId,
				password: password || undefined,
				expires_at: expiresAt ?? undefined,
				max_downloads: Number.isNaN(downloads) ? 0 : downloads,
			});

			const url = `${window.location.origin}/s/${share.token}`;
			setShareUrl(url);
			toast.success(t("files:share_created"));
		} catch (error) {
			handleApiError(error);
		} finally {
			setLoading(false);
		}
	};

	const handleCopy = async () => {
		if (!shareUrl) return;
		await navigator.clipboard.writeText(shareUrl);
		toast.success(t("common:copied_to_clipboard"));
		setCopied(true);
		setTimeout(() => setCopied(false), 2000);
	};

	const handleClose = (open: boolean) => {
		if (!open) {
			setPassword("");
			setExpiry("never");
			setMaxDownloads("");
			setShareUrl(null);
			setCopied(false);
		}
		onOpenChange(open);
	};

	return (
		<Dialog open={open} onOpenChange={handleClose}>
			<DialogContent className="max-w-md">
				<DialogHeader>
					<DialogTitle className="flex items-center gap-2">
						<Icon name="Link" className="h-4 w-4" />
						{t("files:share_dialog_title", { name })}
					</DialogTitle>
				</DialogHeader>

				{shareUrl ? (
					<div className="space-y-4">
						<div className="flex items-center gap-2">
							<Input value={shareUrl} readOnly className="text-sm" />
							<Button variant="outline" size="icon" onClick={handleCopy}>
								{copied ? (
									<Icon name="Check" className="h-4 w-4 text-green-500" />
								) : (
									<Icon name="Copy" className="h-4 w-4" />
								)}
							</Button>
						</div>
						{password && (
							<p className="text-xs text-muted-foreground">
								{t("files:share_password_hint")}
							</p>
						)}
						<Button
							variant="outline"
							className="w-full"
							onClick={() => handleClose(false)}
						>
							{t("files:share_done")}
						</Button>
					</div>
				) : (
					<form onSubmit={handleCreate} className="space-y-4">
						<div className="space-y-2">
							<Label htmlFor="share-password">
								{t("files:share_password_optional")}
							</Label>
							<Input
								id="share-password"
								type="password"
								placeholder={t("files:share_password_placeholder")}
								value={password}
								onChange={(e) => setPassword(e.target.value)}
							/>
						</div>

						<div className="space-y-2">
							<Label>{t("files:share_expiration")}</Label>
							<Select
								value={expiry}
								onValueChange={(v) => setExpiry(v ?? "never")}
							>
								<SelectTrigger>
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									<SelectItem value="never">
										{t("files:share_expiry_never")}
									</SelectItem>
									<SelectItem value="1h">
										{t("files:share_expiry_1h")}
									</SelectItem>
									<SelectItem value="1d">
										{t("files:share_expiry_1d")}
									</SelectItem>
									<SelectItem value="7d">
										{t("files:share_expiry_7d")}
									</SelectItem>
									<SelectItem value="30d">
										{t("files:share_expiry_30d")}
									</SelectItem>
								</SelectContent>
							</Select>
						</div>

						<div className="space-y-2">
							<Label htmlFor="max-downloads">
								{t("files:share_download_limit")}
							</Label>
							<Input
								id="max-downloads"
								type="number"
								placeholder={t("files:share_download_limit_placeholder")}
								value={maxDownloads}
								onChange={(e) => setMaxDownloads(e.target.value)}
							/>
						</div>

						<Button type="submit" className="w-full" disabled={loading}>
							{loading
								? t("files:share_creating")
								: t("files:share_create_button")}
						</Button>
					</form>
				)}
			</DialogContent>
		</Dialog>
	);
}

function computeExpiry(value: string): string | null {
	if (value === "never") return null;
	const now = new Date();
	switch (value) {
		case "1h":
			now.setHours(now.getHours() + 1);
			break;
		case "1d":
			now.setDate(now.getDate() + 1);
			break;
		case "7d":
			now.setDate(now.getDate() + 7);
			break;
		case "30d":
			now.setDate(now.getDate() + 30);
			break;
		default:
			return null;
	}
	return now.toISOString();
}
