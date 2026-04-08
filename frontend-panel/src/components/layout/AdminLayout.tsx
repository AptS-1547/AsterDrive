import type { ReactNode } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { NavLink } from "react-router-dom";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { AdminTopBar } from "@/components/layout/AdminTopBar";
import { Icon, type IconName } from "@/components/ui/icon";
import { ScrollArea } from "@/components/ui/scroll-area";
import { handleApiError } from "@/hooks/useApiError";
import {
	ADMIN_SIDEBAR_WIDTH_CLASS,
	ADMIN_TOPBAR_OFFSET_CLASS,
	SIDEBAR_SECTION_PADDING_CLASS,
} from "@/lib/constants";
import { normalizePublicSiteUrl, setPublicSiteUrl } from "@/lib/publicSiteUrl";
import { cn, sidebarNavItemClass } from "@/lib/utils";
import { adminConfigService } from "@/services/adminService";
import { useBrandingStore } from "@/stores/brandingStore";

const PUBLIC_SITE_URL_KEY = "public_site_url";

function syncPublicSiteUrlRuntime(value: string | null | undefined) {
	const siteUrl = setPublicSiteUrl(value);
	useBrandingStore.setState({ siteUrl });
}

export function AdminLayout({ children }: { children: ReactNode }) {
	const { t } = useTranslation("admin");
	const isBrandingLoaded = useBrandingStore((state) => state.isLoaded);
	const configuredSiteUrl = useBrandingStore((state) => state.siteUrl);
	const [mobileOpen, setMobileOpen] = useState(false);
	const siteUrlPromptCheckedRef = useRef(false);
	const [siteUrlMismatchDialogOpen, setSiteUrlMismatchDialogOpen] =
		useState(false);
	const [siteUrlMismatchCurrentOrigin, setSiteUrlMismatchCurrentOrigin] =
		useState<string | null>(null);

	const handleMobileToggle = useCallback(() => {
		setMobileOpen((prev) => !prev);
	}, []);

	const handleMobileClose = useCallback(() => {
		setMobileOpen(false);
	}, []);

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
		if (!currentOrigin || configuredSiteUrl === currentOrigin) {
			return;
		}

		setSiteUrlMismatchCurrentOrigin(currentOrigin);
		setSiteUrlMismatchDialogOpen(true);
	}, [configuredSiteUrl, isBrandingLoaded]);

	const handleUpdatePublicSiteUrl = useCallback(async () => {
		if (!siteUrlMismatchCurrentOrigin) {
			return;
		}

		try {
			const savedConfig = await adminConfigService.set(
				PUBLIC_SITE_URL_KEY,
				siteUrlMismatchCurrentOrigin,
			);
			syncPublicSiteUrlRuntime(savedConfig.value);
			toast.success(t("settings_saved"));
		} catch (error) {
			handleApiError(error);
		}
	}, [siteUrlMismatchCurrentOrigin, t]);

	const primaryNavItems: { to: string; label: string; icon: IconName }[] = [
		{ to: "/admin/overview", label: t("overview"), icon: "Presentation" },
		{ to: "/admin/users", label: t("users"), icon: "Shield" },
		{ to: "/admin/teams", label: t("teams"), icon: "Cloud" },
		{ to: "/admin/policies", label: t("policies"), icon: "HardDrive" },
		{
			to: "/admin/policy-groups",
			label: t("policy_groups"),
			icon: "ListBullets",
		},
		{ to: "/admin/shares", label: t("shares"), icon: "Link" },
		{ to: "/admin/locks", label: t("locks"), icon: "Lock" },
		{ to: "/admin/settings", label: t("system_settings"), icon: "Gear" },
		{
			to: "/admin/audit",
			label: t("audit_log"),
			icon: "ClipboardText",
		},
	];
	const secondaryNavItems: { to: string; label: string; icon: IconName }[] = [
		{ to: "/admin/about", label: t("about"), icon: "Info" },
	];

	const sidebarContent = (
		<div className="flex h-full flex-col bg-background">
			<ScrollArea className="min-h-0 flex-1 pt-2">
				<nav className={cn("space-y-1 py-2", SIDEBAR_SECTION_PADDING_CLASS)}>
					{primaryNavItems.map((item) => (
						<NavLink
							key={item.to}
							to={item.to}
							viewTransition
							onClick={handleMobileClose}
							className={({ isActive }) => sidebarNavItemClass(isActive)}
						>
							<Icon name={item.icon} className="h-4 w-4 shrink-0" />
							{item.label}
						</NavLink>
					))}
				</nav>
			</ScrollArea>
			<div className={cn("border-t py-2", SIDEBAR_SECTION_PADDING_CLASS)}>
				<nav className="space-y-1">
					{secondaryNavItems.map((item) => (
						<NavLink
							key={item.to}
							to={item.to}
							viewTransition
							onClick={handleMobileClose}
							className={({ isActive }) => sidebarNavItemClass(isActive)}
						>
							<Icon name={item.icon} className="h-4 w-4 shrink-0" />
							{item.label}
						</NavLink>
					))}
				</nav>
			</div>
		</div>
	);

	return (
		<div className="flex h-screen flex-col bg-muted/20">
			<AdminTopBar onSidebarToggle={handleMobileToggle} />
			<div className="flex min-h-0 flex-1 overflow-hidden">
				{mobileOpen && (
					<button
						type="button"
						className={cn(
							"fixed inset-x-0 z-40 bg-black/50 md:hidden",
							ADMIN_TOPBAR_OFFSET_CLASS,
						)}
						onClick={handleMobileClose}
						aria-label={t("core:close_admin_sidebar")}
					/>
				)}
				<aside
					className={cn(
						"border-r bg-background transition-all duration-200",
						"hidden shrink-0 md:flex md:flex-col",
						ADMIN_SIDEBAR_WIDTH_CLASS,
						mobileOpen &&
							cn(
								"fixed left-0 z-50 flex flex-col shadow-lg md:relative md:inset-y-0 md:left-auto md:z-auto",
								ADMIN_SIDEBAR_WIDTH_CLASS,
								ADMIN_TOPBAR_OFFSET_CLASS,
							),
					)}
				>
					{sidebarContent}
				</aside>
				<main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
					{children}
				</main>
			</div>
			<ConfirmDialog
				open={siteUrlMismatchDialogOpen}
				onOpenChange={setSiteUrlMismatchDialogOpen}
				title={t("site_url_mismatch_title")}
				description={
					siteUrlMismatchCurrentOrigin
						? t("site_url_mismatch_description", {
								configured: configuredSiteUrl ?? t("site_url_mismatch_not_set"),
								current: siteUrlMismatchCurrentOrigin,
							})
						: undefined
				}
				confirmLabel={t("site_url_mismatch_confirm")}
				onConfirm={() => {
					void handleUpdatePublicSiteUrl();
				}}
			/>
		</div>
	);
}
