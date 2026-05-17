import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import type { AdminExternalAuthProviderInfo } from "@/types/api";
import { CallbackUrlField, callbackUrl } from "./shared";

interface ExternalAuthCallbackDialogProps {
	onCopy: (value: string) => void;
	onOpenChange: (open: boolean) => void;
	provider: AdminExternalAuthProviderInfo | null;
}

export function ExternalAuthCallbackDialog({
	onCopy,
	onOpenChange,
	provider,
}: ExternalAuthCallbackDialogProps) {
	const { t } = useTranslation("admin");
	const value = provider
		? callbackUrl(provider.provider_kind, provider.key)
		: "";

	return (
		<Dialog open={Boolean(provider)} onOpenChange={onOpenChange}>
			<DialogContent className="max-w-[calc(100vw-2rem)] overflow-hidden sm:max-w-xl">
				<DialogHeader>
					<DialogTitle>
						{t("external_auth_provider_created_callback_title")}
					</DialogTitle>
					<DialogDescription>
						{t("external_auth_provider_created_callback_desc", {
							name: provider?.display_name ?? "",
						})}
					</DialogDescription>
				</DialogHeader>
				<div className="min-w-0 max-w-full space-y-2 overflow-hidden">
					<Label>{t("external_auth_provider_callback_url")}</Label>
					<CallbackUrlField value={value} onCopy={onCopy} />
					<p className="text-xs text-muted-foreground">
						{t("external_auth_provider_callback_url_hint")}
					</p>
				</div>
				<DialogFooter>
					<Button
						type="button"
						className={ADMIN_CONTROL_HEIGHT_CLASS}
						onClick={() => onOpenChange(false)}
					>
						{t("core:close")}
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
