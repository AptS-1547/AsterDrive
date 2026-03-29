import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
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
import type { MyShareInfo } from "@/types/api";

type PasswordAction = "keep" | "clear" | "set";

interface EditShareDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	share: MyShareInfo | null;
	onSaved?: () => void | Promise<void>;
}

function toDateTimeLocalValue(value: string | null | undefined): string {
	if (!value) return "";

	const date = new Date(value);
	if (Number.isNaN(date.getTime())) {
		return "";
	}

	const offsetMs = date.getTimezoneOffset() * 60 * 1000;
	return new Date(date.getTime() - offsetMs).toISOString().slice(0, 16);
}

function toIsoDateTime(value: string): string | null {
	const trimmed = value.trim();
	if (!trimmed) return null;

	const date = new Date(trimmed);
	return Number.isNaN(date.getTime()) ? null : date.toISOString();
}

function normalizeMaxDownloads(value: string): number {
	const parsed = Number.parseInt(value, 10);
	if (Number.isNaN(parsed) || parsed < 0) {
		return 0;
	}
	return parsed;
}

export function EditShareDialog({
	open,
	onOpenChange,
	share,
	onSaved,
}: EditShareDialogProps) {
	const { t } = useTranslation(["core", "share"]);
	const [passwordAction, setPasswordAction] = useState<PasswordAction>("keep");
	const [password, setPassword] = useState("");
	const [expiresAt, setExpiresAt] = useState("");
	const [maxDownloads, setMaxDownloads] = useState("0");
	const [loading, setLoading] = useState(false);

	useEffect(() => {
		if (!open || !share) return;

		setPasswordAction("keep");
		setPassword("");
		setExpiresAt(toDateTimeLocalValue(share.expires_at));
		setMaxDownloads(String(share.max_downloads));
	}, [open, share]);

	if (!share) return null;

	const handleSave = async (event: React.FormEvent) => {
		event.preventDefault();

		if (passwordAction === "set" && password.trim().length === 0) {
			toast.error(t("share:share_edit_password_required"));
			return;
		}

		setLoading(true);
		try {
			await shareService.update(share.id, {
				password:
					passwordAction === "keep"
						? undefined
						: passwordAction === "clear"
							? ""
							: password.trim(),
				expires_at: toIsoDateTime(expiresAt),
				max_downloads: normalizeMaxDownloads(maxDownloads),
			});
			toast.success(t("share:my_shares_edit_success"));
			onOpenChange(false);
			await onSaved?.();
		} catch (error) {
			handleApiError(error);
		} finally {
			setLoading(false);
		}
	};

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="max-w-md">
				<DialogHeader>
					<DialogTitle className="flex items-center gap-2">
						<Icon name="PencilSimple" className="h-4 w-4" />
						{t("share:my_shares_edit_title", { name: share.resource_name })}
					</DialogTitle>
					<DialogDescription>
						{t("share:my_shares_edit_desc")}
					</DialogDescription>
				</DialogHeader>

				<form onSubmit={handleSave} className="space-y-4">
					<div className="space-y-2">
						<Label>{t("share:my_shares_edit_password_mode")}</Label>
						<Select
							value={passwordAction}
							onValueChange={(value) =>
								setPasswordAction((value as PasswordAction | null) ?? "keep")
							}
						>
							<SelectTrigger>
								<SelectValue />
							</SelectTrigger>
							<SelectContent>
								<SelectItem value="keep">
									{t("share:my_shares_edit_password_keep")}
								</SelectItem>
								<SelectItem value="clear">
									{t("share:my_shares_edit_password_clear")}
								</SelectItem>
								<SelectItem value="set">
									{t("share:my_shares_edit_password_set")}
								</SelectItem>
							</SelectContent>
						</Select>
					</div>

					{passwordAction === "set" && (
						<div className="space-y-2">
							<Label htmlFor="edit-share-password">
								{t("share:share_password_optional")}
							</Label>
							<Input
								id="edit-share-password"
								type="password"
								placeholder={t("share:share_password_placeholder")}
								value={password}
								onChange={(event) => setPassword(event.target.value)}
							/>
						</div>
					)}

					<div className="space-y-2">
						<Label htmlFor="edit-share-expires-at">
							{t("share:share_expiration")}
						</Label>
						<Input
							id="edit-share-expires-at"
							type="datetime-local"
							value={expiresAt}
							onChange={(event) => setExpiresAt(event.target.value)}
						/>
						<p className="text-xs text-muted-foreground">
							{t("share:my_shares_edit_expiry_hint")}
						</p>
					</div>

					<div className="space-y-2">
						<Label htmlFor="edit-share-max-downloads">
							{t("share:share_download_limit")}
						</Label>
						<Input
							id="edit-share-max-downloads"
							type="number"
							min={0}
							placeholder={t("share:share_download_limit_placeholder")}
							value={maxDownloads}
							onChange={(event) => setMaxDownloads(event.target.value)}
						/>
					</div>

					<div className="flex items-center justify-end gap-2 pt-2">
						<Button
							type="button"
							variant="outline"
							onClick={() => onOpenChange(false)}
						>
							{t("core:cancel")}
						</Button>
						<Button type="submit" disabled={loading}>
							{loading ? t("core:save") : t("core:save")}
						</Button>
					</div>
				</form>
			</DialogContent>
		</Dialog>
	);
}
