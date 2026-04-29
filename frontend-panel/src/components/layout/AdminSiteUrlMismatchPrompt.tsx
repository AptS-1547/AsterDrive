import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { handleApiError } from "@/hooks/useApiError";
import {
	getPublicSiteUrls,
	normalizePublicSiteUrl,
	publicSiteUrlMatches,
	setPublicSiteUrl,
} from "@/lib/publicSiteUrl";
import { adminConfigService } from "@/services/adminService";
import { useBrandingStore } from "@/stores/brandingStore";

const PUBLIC_SITE_URL_KEY = "public_site_url";

function syncPublicSiteUrlRuntime(value: string | null | undefined) {
	const siteUrl = setPublicSiteUrl(value);
	useBrandingStore.setState({ siteUrl });
}

export function AdminSiteUrlMismatchPrompt() {
	const { t } = useTranslation("admin");
	const isBrandingLoaded = useBrandingStore((state) => state.isLoaded);
	const configuredSiteUrl = useBrandingStore((state) => state.siteUrl);
	const siteUrlPromptCheckedRef = useRef(false);
	const [siteUrlMismatchDialogOpen, setSiteUrlMismatchDialogOpen] =
		useState(false);
	const [siteUrlMismatchCurrentOrigin, setSiteUrlMismatchCurrentOrigin] =
		useState<string | null>(null);
	const configuredSiteUrlList = getPublicSiteUrls();
	const configuredSiteUrlDescription =
		configuredSiteUrlList.length > 0
			? configuredSiteUrlList.join(", ")
			: configuredSiteUrl;

	useEffect(() => {
		if (
			siteUrlPromptCheckedRef.current ||
			!isBrandingLoaded ||
			typeof window === "undefined"
		) {
			return;
		}

		siteUrlPromptCheckedRef.current = true;

		const currentOrigin = normalizePublicSiteUrl(window.location.origin);
		if (!currentOrigin || publicSiteUrlMatches(currentOrigin)) {
			return;
		}

		setSiteUrlMismatchCurrentOrigin(currentOrigin);
		setSiteUrlMismatchDialogOpen(true);
	}, [isBrandingLoaded]);

	const handleUpdatePublicSiteUrl = useCallback(async () => {
		if (!siteUrlMismatchCurrentOrigin) {
			return;
		}

		try {
			const nextValue = [
				...getPublicSiteUrls().filter(
					(origin) => origin !== siteUrlMismatchCurrentOrigin,
				),
				siteUrlMismatchCurrentOrigin,
			].join("\n");
			const savedConfig = await adminConfigService.set(
				PUBLIC_SITE_URL_KEY,
				nextValue || siteUrlMismatchCurrentOrigin,
			);
			syncPublicSiteUrlRuntime(savedConfig.value);
			toast.success(t("settings_saved"));
		} catch (error) {
			handleApiError(error);
		}
	}, [siteUrlMismatchCurrentOrigin, t]);

	return (
		<ConfirmDialog
			open={siteUrlMismatchDialogOpen}
			onOpenChange={setSiteUrlMismatchDialogOpen}
			title={t("site_url_mismatch_title")}
			description={
				siteUrlMismatchCurrentOrigin
					? t("site_url_mismatch_description", {
							configured:
								configuredSiteUrlDescription ?? t("site_url_mismatch_not_set"),
							current: siteUrlMismatchCurrentOrigin,
						})
					: undefined
			}
			confirmLabel={t("site_url_mismatch_confirm")}
			onConfirm={() => {
				void handleUpdatePublicSiteUrl();
			}}
		/>
	);
}
