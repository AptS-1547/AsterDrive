import {
	type ReactNode,
	useCallback,
	useEffect,
	useEffectEvent,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { PreviewAppsConfigEditor } from "@/components/admin/PreviewAppsConfigEditor";
import {
	getPreviewAppsConfigIssuesFromString,
	PREVIEW_APPS_CONFIG_KEY,
} from "@/components/admin/previewAppsConfigEditorShared";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { CodePreviewEditor } from "@/components/files/preview/CodePreviewEditor";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { AdminPageHeader } from "@/components/layout/AdminPageHeader";
import { AdminPageShell } from "@/components/layout/AdminPageShell";
import { Button, buttonVariants } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuRadioGroup,
	DropdownMenuRadioItem,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Icon, type IconName } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { handleApiError } from "@/hooks/useApiError";
import { usePageTitle } from "@/hooks/usePageTitle";
import {
	ADMIN_SETTINGS_CONTENT_BASE_BOTTOM_PADDING_DESKTOP_PX,
	ADMIN_SETTINGS_CONTENT_BASE_BOTTOM_PADDING_MOBILE_PX,
	ADMIN_SETTINGS_SAVE_BAR_MIN_RESERVED_HEIGHT_DESKTOP_PX,
	ADMIN_SETTINGS_SAVE_BAR_MIN_RESERVED_HEIGHT_MOBILE_PX,
} from "@/lib/constants";
import { setPublicSiteUrl } from "@/lib/publicSiteUrl";
import { cn } from "@/lib/utils";
import { adminConfigService } from "@/services/adminService";
import { useAuthStore } from "@/stores/authStore";
import { useBrandingStore } from "@/stores/brandingStore";
import { useThemeStore } from "@/stores/themeStore";
import type {
	ConfigSchemaItem,
	SystemConfig,
	TemplateVariableGroup,
} from "@/types/api";

const CATEGORY_ORDER = [
	"general",
	"user",
	"auth",
	"mail",
	"network",
	"operations",
	"storage",
	"webdav",
	"audit",
	"custom",
	"other",
] as const;

const CATEGORY_INDEX: Record<string, number> = Object.fromEntries(
	CATEGORY_ORDER.map((category, index) => [category, index]),
);

const MOBILE_BREAKPOINT = 768;
const DESKTOP_NAV_BREAKPOINT = 1280;
const COMPACT_NAV_TAB_GAP = 8;
const COMPACT_NAV_OVERFLOW_GAP = 12;
const SAVE_BAR_ENTER_DURATION_MS = 180;
const SAVE_BAR_EXIT_DURATION_MS = 140;
const TEMPLATE_GROUP_EXPAND_DURATION_MS = 280;
const TEMPLATE_GROUP_COLLAPSE_DURATION_MS = 240;
const TEMPLATE_GROUP_EXPAND_EASING = "cubic-bezier(0.22, 1, 0.36, 1)";
const TEMPLATE_GROUP_COLLAPSE_EASING = "cubic-bezier(0.32, 0, 0.67, 0.96)";
const COMPACT_NAV_TAB_TRIGGER_CLASS =
	"h-10 flex-none rounded-none px-0 text-sm font-medium";
const COMPACT_NAV_TAB_CONTENT_CLASS =
	"inline-flex items-center gap-2 px-3 py-2";
const COMPACT_NAV_OVERFLOW_TRIGGER_CLASS = buttonVariants({
	variant: "secondary",
	size: "sm",
	className: "shrink-0 rounded-full px-3",
});

const PUBLIC_SITE_URL_KEY = "public_site_url";
type BrandingAssetPreviewAppearance = {
	fallbackLabel: string;
	frameClassName: string;
	imageClassName: string;
	validClassName: string;
	validHoverClassName: string;
};

const BRANDING_ASSET_PREVIEW_APPEARANCES: Record<
	string,
	BrandingAssetPreviewAppearance
> = {
	branding_favicon_url: {
		fallbackLabel: "/favicon.svg",
		frameClassName: "w-12",
		imageClassName: "max-h-8 max-w-8 object-contain",
		validClassName: "border-neutral-200 bg-white",
		validHoverClassName: "hover:border-primary/40 hover:bg-neutral-50",
	},
	branding_wordmark_dark_url: {
		fallbackLabel: "/static/asterdrive/asterdrive-dark.svg",
		frameClassName: "w-36 px-4",
		imageClassName: "max-h-7 w-full object-contain",
		validClassName: "border-neutral-200 bg-white",
		validHoverClassName: "hover:border-primary/40 hover:bg-neutral-50",
	},
	branding_wordmark_light_url: {
		fallbackLabel: "/static/asterdrive/asterdrive-light.svg",
		frameClassName: "w-36 px-4",
		imageClassName: "max-h-7 w-full object-contain",
		validClassName: "border-neutral-700 bg-black",
		validHoverClassName: "hover:border-primary/50 hover:bg-neutral-950",
	},
};

type DraftValues = Record<string, string>;
type TimeConfigBaseUnit = "seconds" | "hours" | "days";
type TimeDisplayUnitValue = "seconds" | "minutes" | "hours" | "days" | "weeks";
type SizeDisplayUnitValue =
	| "bytes"
	| "kilobytes"
	| "megabytes"
	| "gigabytes"
	| "terabytes";
type DisplayUnit = {
	labelKey: string;
	multiplier: number;
	value: string;
};

type NewCustomDraft = {
	id: string;
	key: string;
	value: string;
};

type CategorySummary = {
	category: string;
	description?: string;
	icon: IconName;
	label: string;
};

type CategoryPath = {
	category: string;
	subcategory?: string;
};

type SystemSubcategoryGroup = {
	category: string;
	subcategory?: string;
	configs: SystemConfig[];
};

const TIME_DISPLAY_UNITS: Record<TimeConfigBaseUnit, readonly DisplayUnit[]> = {
	seconds: [
		{
			value: "days",
			labelKey: "settings_time_unit_days",
			multiplier: 24 * 60 * 60,
		},
		{
			value: "hours",
			labelKey: "settings_time_unit_hours",
			multiplier: 60 * 60,
		},
		{
			value: "minutes",
			labelKey: "settings_time_unit_minutes",
			multiplier: 60,
		},
		{
			value: "seconds",
			labelKey: "settings_time_unit_seconds",
			multiplier: 1,
		},
	],
	hours: [
		{
			value: "days",
			labelKey: "settings_time_unit_days",
			multiplier: 24,
		},
		{
			value: "hours",
			labelKey: "settings_time_unit_hours",
			multiplier: 1,
		},
	],
	days: [
		{
			value: "weeks",
			labelKey: "settings_time_unit_weeks",
			multiplier: 7,
		},
		{
			value: "days",
			labelKey: "settings_time_unit_days",
			multiplier: 1,
		},
	],
};

const SIZE_DISPLAY_UNITS: readonly DisplayUnit[] = [
	{
		value: "terabytes",
		labelKey: "settings_size_unit_terabytes",
		multiplier: 1024 ** 4,
	},
	{
		value: "gigabytes",
		labelKey: "settings_size_unit_gigabytes",
		multiplier: 1024 ** 3,
	},
	{
		value: "megabytes",
		labelKey: "settings_size_unit_megabytes",
		multiplier: 1024 ** 2,
	},
	{
		value: "kilobytes",
		labelKey: "settings_size_unit_kilobytes",
		multiplier: 1024,
	},
	{
		value: "bytes",
		labelKey: "settings_size_unit_bytes",
		multiplier: 1,
	},
];

const SIZE_CONFIG_KEYS = new Set(["default_storage_quota"]);

export type AdminSettingsTab = (typeof CATEGORY_ORDER)[number];
type SaveBarPhase = "hidden" | "entering" | "visible" | "exiting";

function syncPublicSiteUrlRuntime(value: string | null | undefined) {
	const siteUrl = setPublicSiteUrl(value);
	useBrandingStore.setState({ siteUrl });
}

function getCategoryIcon(category: string): IconName {
	switch (category) {
		case "user":
			return "User";
		case "auth":
			return "Shield";
		case "network":
			return "Globe";
		case "operations":
			return "Clock";
		case "mail":
			return "EnvelopeSimple";
		case "storage":
			return "HardDrive";
		case "webdav":
			return "FolderOpen";
		case "audit":
			return "Scroll";
		case "general":
			return "Gear";
		case "custom":
			return "BracketsCurly";
		default:
			return "Grid";
	}
}

function getAdminSettingsSectionTitle(
	section: AdminSettingsTab,
	t: ReturnType<typeof useTranslation>["t"],
) {
	switch (section) {
		case "user":
			return t("settings_category_user");
		case "auth":
			return t("settings_category_auth");
		case "network":
			return t("settings_category_network");
		case "operations":
			return t("settings_category_operations");
		case "mail":
			return t("settings_category_mail");
		case "storage":
			return t("settings_category_storage");
		case "webdav":
			return t("settings_category_webdav");
		case "audit":
			return t("settings_category_audit");
		case "custom":
			return t("settings_category_custom");
		case "other":
			return t("settings_category_other");
		default:
			return t("settings_category_general");
	}
}

function getConfigDescription(config: SystemConfig) {
	return config.description || undefined;
}

function getConfigIsSensitive(config: SystemConfig) {
	return config.is_sensitive ?? false;
}

function getConfigRequiresRestart(config: SystemConfig) {
	return config.requires_restart ?? false;
}

function getConfigValueType(config: SystemConfig) {
	return config.value_type ?? "string";
}

function isNumberType(valueType: string) {
	return valueType === "number";
}

function isMultilineType(valueType: string) {
	return valueType === "multiline";
}

function isBrandingAssetConfig(config: SystemConfig) {
	return config.key in BRANDING_ASSET_PREVIEW_APPEARANCES;
}

function getBrandingAssetPreviewAppearance(config: SystemConfig) {
	return BRANDING_ASSET_PREVIEW_APPEARANCES[config.key];
}

function getTimeConfigBaseUnit(
	config: SystemConfig,
): TimeConfigBaseUnit | null {
	if (!isNumberType(getConfigValueType(config))) {
		return null;
	}
	if (config.key.endsWith("_secs")) {
		return "seconds";
	}
	if (config.key.endsWith("_hours")) {
		return "hours";
	}
	if (config.key.endsWith("_days")) {
		return "days";
	}
	return null;
}

function isSizeConfig(config: SystemConfig) {
	return (
		isNumberType(getConfigValueType(config)) &&
		(config.key.endsWith("_bytes") || SIZE_CONFIG_KEYS.has(config.key))
	);
}

function parseWholeNumber(value: string) {
	const trimmed = value.trim();
	if (!trimmed) {
		return null;
	}
	if (!/^-?\d+$/.test(trimmed)) {
		return null;
	}

	const parsed = Number(trimmed);
	return Number.isSafeInteger(parsed) ? parsed : null;
}

function getAvailableDisplayUnits<T extends DisplayUnit>(
	units: readonly T[],
	value: string,
) {
	const parsed = parseWholeNumber(value);

	if (parsed === null) {
		return units;
	}

	return units.filter(
		(unit) => unit.multiplier === 1 || parsed % unit.multiplier === 0,
	);
}

function getPreferredDisplayUnit<T extends DisplayUnit>(
	units: readonly T[],
	value: string,
) {
	const parsed = parseWholeNumber(value);
	if (parsed === 0) {
		return units[units.length - 1];
	}

	return getAvailableDisplayUnits(units, value)[0] ?? units[units.length - 1];
}

function formatDisplayValue(value: string, unit: DisplayUnit) {
	if (!value.trim()) {
		return "";
	}

	const parsed = parseWholeNumber(value);
	if (parsed === null) {
		return value;
	}

	return String(parsed / unit.multiplier);
}

function normalizeAssetPreviewUrl(value: string) {
	const normalized = value.trim();
	if (!normalized || normalized.includes(" ")) {
		return null;
	}
	if (normalized.startsWith("/") && !normalized.startsWith("//")) {
		return normalized;
	}

	try {
		const resolved = new URL(normalized);
		if (resolved.protocol === "http:" || resolved.protocol === "https:") {
			return resolved.toString();
		}
	} catch {
		return null;
	}

	return null;
}

function UrlAssetPreview({
	url,
	appearance,
}: {
	url: string;
	appearance: BrandingAssetPreviewAppearance;
}) {
	const [debouncedUrl, setDebouncedUrl] = useState(url);

	useEffect(() => {
		const timer = window.setTimeout(() => {
			setDebouncedUrl(url);
		}, 300);
		return () => window.clearTimeout(timer);
	}, [url]);

	const normalizedUrl = useMemo(
		() => normalizeAssetPreviewUrl(debouncedUrl),
		[debouncedUrl],
	);
	const isInvalid = debouncedUrl.trim().length > 0 && !normalizedUrl;
	const previewClassName = cn(
		"group flex h-12 shrink-0 items-center justify-center overflow-hidden rounded-xl border transition-colors",
		appearance.frameClassName,
		normalizedUrl
			? [appearance.validClassName, appearance.validHoverClassName]
			: isInvalid
				? "border-amber-300/70 bg-amber-50/70"
				: appearance.validClassName,
	);

	const previewContent = normalizedUrl ? (
		<UrlAssetPreviewImage
			key={normalizedUrl}
			data-testid="branding-asset-preview-image"
			className={appearance.imageClassName}
			url={normalizedUrl}
		/>
	) : (
		<Icon
			name={isInvalid ? "Warning" : "LinkSimple"}
			className={cn(
				"h-4 w-4",
				isInvalid ? "text-amber-600" : "text-muted-foreground",
			)}
		/>
	);

	return (
		<div data-testid="branding-asset-preview" className="shrink-0">
			{normalizedUrl ? (
				<a
					href={normalizedUrl}
					target="_blank"
					rel="noreferrer"
					className={previewClassName}
					title={normalizedUrl}
					aria-label={normalizedUrl}
				>
					{previewContent}
				</a>
			) : (
				<div
					role="img"
					className={previewClassName}
					title={debouncedUrl.trim() || appearance.fallbackLabel}
					aria-label={debouncedUrl.trim() || appearance.fallbackLabel}
				>
					{previewContent}
				</div>
			)}
		</div>
	);
}

function UrlAssetPreviewImage({
	className,
	url,
	...props
}: {
	className?: string;
	url: string;
	"data-testid"?: string;
}) {
	const [hasLoadError, setHasLoadError] = useState(false);

	if (hasLoadError) {
		return <Icon name="Warning" className="h-5 w-5 text-amber-600" />;
	}

	return (
		<img
			{...props}
			src={url}
			alt=""
			className={className}
			onError={() => setHasLoadError(true)}
		/>
	);
}

function splitCategoryPath(category?: string): CategoryPath {
	const normalized = category?.trim() || "other";
	const [root, ...rest] = normalized.split(".");
	const subcategory = rest.join(".").trim();

	return {
		category: root || "other",
		subcategory: subcategory || undefined,
	};
}

function normalizeCategory(category?: string) {
	return splitCategoryPath(category).category;
}

function normalizeSubcategory(category?: string) {
	return splitCategoryPath(category).subcategory;
}

function formatSubcategoryLabel(segment: string) {
	return segment
		.split(/[._-]+/)
		.filter(Boolean)
		.map((part) => part[0]?.toUpperCase() + part.slice(1))
		.join(" ");
}

function getSubcategoryGroupKey(category: string, subcategory?: string) {
	return `${category}:${subcategory ?? "__default__"}`;
}

const MAIL_TEMPLATE_GROUP_ORDER = [
	"register_activation",
	"contact_change_confirmation",
	"password_reset",
	"password_reset_notice",
	"contact_change_notice",
] as const;

function getMailTemplateGroupOrderIndex(groupId: string) {
	const index = MAIL_TEMPLATE_GROUP_ORDER.indexOf(
		groupId as (typeof MAIL_TEMPLATE_GROUP_ORDER)[number],
	);
	return index === -1 ? Number.MAX_SAFE_INTEGER : index;
}

function getMailTemplateGroupId(configKey: string) {
	return configKey
		.replace(/^mail_template_/, "")
		.replace(/_(subject|html)$/, "");
}

function getMailTemplateFieldOrder(configKey: string) {
	if (configKey.endsWith("_subject")) {
		return 0;
	}
	if (configKey.endsWith("_html")) {
		return 1;
	}
	return 2;
}

function getConfigEditorLanguage(config: SystemConfig) {
	if (config.key.endsWith("_html")) {
		return "html";
	}
	if (config.key.endsWith("_json") || config.key.endsWith(".json")) {
		return "json";
	}
	return "plaintext";
}

function getEditorLanguageLabel(language: string) {
	switch (language) {
		case "html":
			return "HTML";
		case "json":
			return "JSON";
		default:
			return "TEXT";
	}
}

function ConfigCodeEditor({
	language,
	onChange,
	theme,
	value,
}: {
	language: string;
	onChange: (value: string) => void;
	theme: "vs" | "vs-dark";
	value: string;
}) {
	return (
		<div className="max-w-5xl overflow-hidden rounded-xl border bg-background shadow-sm">
			<div className="flex items-center gap-2 border-b bg-muted/40 px-4 py-2">
				<Icon name="FileCode" className="h-4 w-4 text-muted-foreground" />
				<span className="text-sm font-medium">
					{getEditorLanguageLabel(language)}
				</span>
			</div>
			<div className="h-80 min-h-80 bg-background">
				<CodePreviewEditor
					language={language}
					theme={theme}
					value={value}
					onChange={onChange}
					options={{
						domReadOnly: false,
						fontSize: 13,
						lineNumbers: "on",
						padding: { top: 12 },
						readOnly: false,
						renderLineHighlight: "line",
						scrollBeyondLastLine: false,
						wordWrap: "off",
					}}
				/>
			</div>
		</div>
	);
}

function AnimatedCollapsible({
	children,
	className,
	contentClassName,
	open,
}: {
	children: ReactNode;
	className?: string;
	contentClassName?: string;
	open: boolean;
}) {
	const containerRef = useRef<HTMLDivElement | null>(null);
	const contentRef = useRef<HTMLDivElement | null>(null);
	const [mounted, setMounted] = useState(open);

	useEffect(() => {
		if (typeof window === "undefined") {
			setMounted(open);
			return;
		}

		if (open) {
			setMounted(true);
		}
	}, [open]);

	useLayoutEffect(() => {
		if (typeof window === "undefined") {
			return;
		}
		if (!mounted) {
			return;
		}

		const container = containerRef.current;
		const content = contentRef.current;
		if (!container || !content) {
			return;
		}

		const prefersReducedMotion =
			typeof window.matchMedia === "function" &&
			window.matchMedia("(prefers-reduced-motion: reduce)").matches;
		const duration = prefersReducedMotion
			? 0
			: open
				? TEMPLATE_GROUP_EXPAND_DURATION_MS
				: TEMPLATE_GROUP_COLLAPSE_DURATION_MS;
		let frameA: number | null = null;
		let frameB: number | null = null;
		let timer: number | null = null;
		const fullHeight = `${content.scrollHeight}px`;

		container.style.overflow = "hidden";
		container.style.transitionProperty = "max-height, opacity";
		container.style.transitionDuration = `${duration}ms`;
		container.style.transitionTimingFunction = open
			? TEMPLATE_GROUP_EXPAND_EASING
			: TEMPLATE_GROUP_COLLAPSE_EASING;

		if (open) {
			container.style.maxHeight = "0px";
			container.style.opacity = "0";
			frameA = window.requestAnimationFrame(() => {
				frameB = window.requestAnimationFrame(() => {
					container.style.maxHeight = fullHeight;
					container.style.opacity = "1";
				});
			});
			timer = window.setTimeout(() => {
				container.style.maxHeight = "none";
				container.style.opacity = "1";
			}, duration);
		} else {
			container.style.maxHeight = fullHeight;
			container.style.opacity = "1";
			frameA = window.requestAnimationFrame(() => {
				container.style.maxHeight = "0px";
				container.style.opacity = "0";
			});
			timer = window.setTimeout(() => {
				setMounted(false);
			}, duration);
		}

		return () => {
			if (frameA !== null) {
				window.cancelAnimationFrame(frameA);
			}
			if (frameB !== null) {
				window.cancelAnimationFrame(frameB);
			}
			if (timer !== null) {
				window.clearTimeout(timer);
			}
		};
	}, [mounted, open]);

	if (!mounted) {
		return null;
	}

	return (
		<div
			ref={containerRef}
			aria-hidden={!open}
			className={cn("overflow-hidden", className)}
		>
			<div ref={contentRef} className={cn("min-h-0", contentClassName)}>
				{children}
			</div>
		</div>
	);
}

function sortConfigsByKey(a: SystemConfig, b: SystemConfig) {
	return a.key.localeCompare(b.key);
}

function buildDraftValues(configs: SystemConfig[]) {
	return Object.fromEntries(
		configs.map((config) => [config.key, config.value]),
	) as DraftValues;
}

function areCategoryListsEqual(left: string[], right: string[]) {
	return (
		left.length === right.length &&
		left.every((category, index) => category === right[index])
	);
}

export default function AdminSettingsPage({
	section = CATEGORY_ORDER[0],
}: {
	section?: AdminSettingsTab;
}) {
	const { t } = useTranslation("admin");
	usePageTitle(getAdminSettingsSectionTitle(section, t));
	const navigate = useNavigate();
	const currentUserEmail = useAuthStore((state) => state.user?.email ?? "");
	const editorTheme = useThemeStore((state) =>
		state.resolvedTheme === "dark" ? "vs-dark" : "vs",
	);
	const customDraftIdRef = useRef(0);
	const compactNavContainerRef = useRef<HTMLDivElement | null>(null);
	const compactTabMeasureRefs = useRef<
		Record<string, HTMLButtonElement | null>
	>({});
	const compactOverflowMeasureRefs = useRef<
		Record<string, HTMLButtonElement | null>
	>({});
	const compactOverflowDefaultMeasureRef = useRef<HTMLButtonElement | null>(
		null,
	);
	const saveBarTimerRef = useRef<number | null>(null);
	const saveBarPhaseRef = useRef<SaveBarPhase>("hidden");
	const saveBarMeasureRef = useRef<HTMLDivElement | null>(null);
	const [configs, setConfigs] = useState<SystemConfig[]>([]);
	const [schemas, setSchemas] = useState<ConfigSchemaItem[]>([]);
	const [templateVariableGroups, setTemplateVariableGroups] = useState<
		TemplateVariableGroup[]
	>([]);
	const [loading, setLoading] = useState(true);
	const [saving, setSaving] = useState(false);
	const [viewportWidth, setViewportWidth] = useState(() =>
		typeof window === "undefined" ? DESKTOP_NAV_BREAKPOINT : window.innerWidth,
	);
	const [activeTab, setActiveTab] = useState<string>(section);
	const [tabDirection, setTabDirection] = useState<"forward" | "backward">(
		"forward",
	);
	const [draftValues, setDraftValues] = useState<DraftValues>({});
	const [displayUnits, setDisplayUnits] = useState<
		Partial<Record<string, TimeDisplayUnitValue | SizeDisplayUnitValue>>
	>({});
	const [deletedCustomKeys, setDeletedCustomKeys] = useState<string[]>([]);
	const [newCustomRows, setNewCustomRows] = useState<NewCustomDraft[]>([]);
	const [compactInlineCategories, setCompactInlineCategories] = useState<
		string[]
	>([]);
	const [expandedSubcategoryGroups, setExpandedSubcategoryGroups] = useState<
		Record<string, boolean>
	>({});
	const [expandedTemplateGroups, setExpandedTemplateGroups] = useState<
		Record<string, boolean>
	>({});
	const [saveBarPhase, setSaveBarPhase] = useState<SaveBarPhase>("hidden");
	const [saveBarReservedHeight, setSaveBarReservedHeight] = useState(0);
	const [testEmailDialogOpen, setTestEmailDialogOpen] = useState(false);
	const [activeTemplateVariableGroupCode, setActiveTemplateVariableGroupCode] =
		useState<string | null>(null);
	const [testEmailTarget, setTestEmailTarget] = useState("");
	const [sendingTestEmail, setSendingTestEmail] = useState(false);
	const settingsContentBaseBottomPadding =
		viewportWidth < MOBILE_BREAKPOINT
			? ADMIN_SETTINGS_CONTENT_BASE_BOTTOM_PADDING_MOBILE_PX
			: ADMIN_SETTINGS_CONTENT_BASE_BOTTOM_PADDING_DESKTOP_PX;

	const openTestEmailDialog = useCallback(() => {
		setTestEmailTarget(currentUserEmail);
		setTestEmailDialogOpen(true);
	}, [currentUserEmail]);

	const handleSendTestEmail = useCallback(async () => {
		setSendingTestEmail(true);
		try {
			const response = await adminConfigService.sendTestEmail(
				testEmailTarget.trim() || undefined,
			);
			toast.success(response.message);
			setTestEmailDialogOpen(false);
		} catch (error) {
			handleApiError(error);
		} finally {
			setSendingTestEmail(false);
		}
	}, [testEmailTarget]);

	const handleBuildWopiDiscoveryPreviewConfig = useCallback(
		async ({
			discoveryUrl,
			value,
		}: {
			discoveryUrl: string;
			value: string;
		}) => {
			try {
				const response = await adminConfigService.action(
					PREVIEW_APPS_CONFIG_KEY,
					{
						action: "build_wopi_discovery_preview_config",
						discovery_url: discoveryUrl,
						value,
					},
				);
				toast.success(response.message);
				return response.value ?? value;
			} catch (error) {
				handleApiError(error);
				throw error;
			}
		},
		[],
	);

	const load = useCallback(async (options?: { showLoading?: boolean }) => {
		const showLoading = options?.showLoading ?? true;

		try {
			if (showLoading) {
				setLoading(true);
			}
			const [cfgs, schemaList, nextTemplateVariableGroups] = await Promise.all([
				adminConfigService.list({ limit: 200, offset: 0 }),
				adminConfigService.schema(),
				adminConfigService.templateVariables().catch((error) => {
					handleApiError(error);
					return [];
				}),
			]);
			setConfigs(cfgs.items);
			setSchemas(schemaList);
			setTemplateVariableGroups(nextTemplateVariableGroups);
		} catch (error) {
			handleApiError(error);
		} finally {
			if (showLoading) {
				setLoading(false);
			}
		}
	}, []);

	useEffect(() => {
		if (typeof window === "undefined") return;

		const updateViewportWidth = () => {
			setViewportWidth(window.innerWidth);
		};

		updateViewportWidth();
		window.addEventListener("resize", updateViewportWidth);
		return () => window.removeEventListener("resize", updateViewportWidth);
	}, []);

	useEffect(() => {
		void load();
	}, [load]);

	useEffect(() => {
		setDraftValues(buildDraftValues(configs));
		setDisplayUnits({});
		setDeletedCustomKeys([]);
		setNewCustomRows([]);
	}, [configs]);

	const schemaMap = useMemo(() => {
		const map = new Map<string, ConfigSchemaItem>();
		for (const schema of schemas) {
			map.set(schema.key, schema);
		}
		return map;
	}, [schemas]);

	const resolveSchemaTranslation = useCallback(
		(translationKey: string | undefined, fallback?: string) => {
			if (!translationKey) {
				return fallback;
			}

			const translated = t(translationKey);
			return translated === translationKey ? fallback : translated;
		},
		[t],
	);

	const getSystemConfigLabel = useCallback(
		(config: SystemConfig) => {
			const schema = schemaMap.get(config.key);
			return (
				resolveSchemaTranslation(schema?.label_i18n_key, config.key) ??
				config.key
			);
		},
		[resolveSchemaTranslation, schemaMap],
	);

	const getSystemConfigDescription = useCallback(
		(config: SystemConfig) => {
			const schema = schemaMap.get(config.key);
			const fallback = schema?.description || getConfigDescription(config);
			return resolveSchemaTranslation(schema?.description_i18n_key, fallback);
		},
		[resolveSchemaTranslation, schemaMap],
	);

	const mailTemplateVariableGroups = useMemo(
		() =>
			[...templateVariableGroups]
				.filter((group) => group.category === "mail.template")
				.sort(
					(left, right) =>
						getMailTemplateGroupOrderIndex(left.template_code) -
							getMailTemplateGroupOrderIndex(right.template_code) ||
						left.template_code.localeCompare(right.template_code),
				),
		[templateVariableGroups],
	);

	const activeTemplateVariableGroup = useMemo(
		() =>
			activeTemplateVariableGroupCode
				? (mailTemplateVariableGroups.find(
						(group) => group.template_code === activeTemplateVariableGroupCode,
					) ?? null)
				: null,
		[activeTemplateVariableGroupCode, mailTemplateVariableGroups],
	);

	const getTemplateVariableGroupLabel = useCallback(
		(group: TemplateVariableGroup) =>
			resolveSchemaTranslation(
				group.label_i18n_key,
				formatSubcategoryLabel(group.template_code),
			) ?? formatSubcategoryLabel(group.template_code),
		[resolveSchemaTranslation],
	);

	const getTemplateVariableLabel = useCallback(
		(variable: TemplateVariableGroup["variables"][number]) =>
			resolveSchemaTranslation(variable.label_i18n_key, variable.token) ??
			variable.token,
		[resolveSchemaTranslation],
	);

	const getTemplateVariableDescription = useCallback(
		(variable: TemplateVariableGroup["variables"][number]) =>
			resolveSchemaTranslation(variable.description_i18n_key),
		[resolveSchemaTranslation],
	);

	const openTemplateVariablesDialog = useCallback((config: SystemConfig) => {
		setActiveTemplateVariableGroupCode(getMailTemplateGroupId(config.key));
	}, []);

	const systemConfigs = useMemo(
		() =>
			configs
				.filter((config) => config.source === "system")
				.sort(sortConfigsByKey),
		[configs],
	);

	const customConfigs = useMemo(
		() =>
			configs
				.filter((config) => config.source !== "system")
				.sort(sortConfigsByKey),
		[configs],
	);

	const systemGroups = useMemo(() => {
		const groups: Record<string, SystemConfig[]> = {};

		for (const config of systemConfigs) {
			const category = normalizeCategory(config.category);
			if (!groups[category]) {
				groups[category] = [];
			}
			groups[category].push(config);
		}

		return groups;
	}, [systemConfigs]);

	const systemCategories = useMemo(
		() =>
			Object.keys(systemGroups).sort((left, right) => {
				const leftIndex = CATEGORY_INDEX[left] ?? Number.MAX_SAFE_INTEGER;
				const rightIndex = CATEGORY_INDEX[right] ?? Number.MAX_SAFE_INTEGER;
				return leftIndex - rightIndex || left.localeCompare(right);
			}),
		[systemGroups],
	);

	const systemSubcategoryGroups = useMemo(() => {
		const groups: Record<string, SystemSubcategoryGroup[]> = {};

		for (const category of systemCategories) {
			const grouped = new Map<string, SystemSubcategoryGroup>();

			for (const config of systemGroups[category] ?? []) {
				const subcategory = normalizeSubcategory(config.category);
				const groupKey = getSubcategoryGroupKey(category, subcategory);
				const existingGroup = grouped.get(groupKey);
				if (existingGroup) {
					existingGroup.configs.push(config);
					continue;
				}

				grouped.set(groupKey, {
					category,
					subcategory,
					configs: [config],
				});
			}

			groups[category] = Array.from(grouped.values()).sort((left, right) => {
				if (!left.subcategory && !right.subcategory) return 0;
				if (!left.subcategory) return -1;
				if (!right.subcategory) return 1;
				return left.subcategory.localeCompare(right.subcategory);
			});
		}

		return groups;
	}, [systemCategories, systemGroups]);

	const tabCategories = useMemo(() => {
		const categories = [...systemCategories];
		if (!categories.includes("custom")) {
			categories.push("custom");
		}
		return categories;
	}, [systemCategories]);

	const resolvedSection = useMemo(() => {
		if (tabCategories.includes(section)) {
			return section;
		}

		return tabCategories[0] ?? section;
	}, [section, tabCategories]);

	const deletedCustomKeySet = useMemo(
		() => new Set(deletedCustomKeys),
		[deletedCustomKeys],
	);

	const visibleCustomConfigs = useMemo(
		() =>
			customConfigs.filter((config) => !deletedCustomKeySet.has(config.key)),
		[customConfigs, deletedCustomKeySet],
	);

	const deletedCustomConfigs = useMemo(
		() => customConfigs.filter((config) => deletedCustomKeySet.has(config.key)),
		[customConfigs, deletedCustomKeySet],
	);

	const activeNewCustomRows = useMemo(
		() =>
			newCustomRows.filter(
				(row) => row.key.trim().length > 0 || row.value.trim().length > 0,
			),
		[newCustomRows],
	);

	const newCustomRowErrors = useMemo(() => {
		const errors = new Map<string, string>();
		const keyCounts = new Map<string, number>();

		for (const row of activeNewCustomRows) {
			const trimmedKey = row.key.trim();
			if (!trimmedKey) continue;
			keyCounts.set(trimmedKey, (keyCounts.get(trimmedKey) ?? 0) + 1);
		}

		const existingKeys = new Set(
			visibleCustomConfigs.map((config) => config.key),
		);

		for (const row of activeNewCustomRows) {
			const trimmedKey = row.key.trim();
			if (!trimmedKey) {
				errors.set(row.id, t("custom_config_key_required"));
				continue;
			}
			if (
				existingKeys.has(trimmedKey) ||
				(keyCounts.get(trimmedKey) ?? 0) > 1
			) {
				errors.set(row.id, t("custom_config_key_duplicate"));
			}
		}

		return errors;
	}, [activeNewCustomRows, t, visibleCustomConfigs]);

	const changedExistingConfigs = useMemo(
		() =>
			configs.filter((config) => {
				if (deletedCustomKeySet.has(config.key)) {
					return false;
				}
				return (draftValues[config.key] ?? config.value) !== config.value;
			}),
		[configs, deletedCustomKeySet, draftValues],
	);

	const previewAppsValidationIssues = useMemo(() => {
		const config = configs.find((item) => item.key === PREVIEW_APPS_CONFIG_KEY);
		if (!config) {
			return [];
		}

		return getPreviewAppsConfigIssuesFromString(
			draftValues[config.key] ?? config.value,
		);
	}, [configs, draftValues]);

	const changedCount =
		changedExistingConfigs.length +
		deletedCustomConfigs.length +
		activeNewCustomRows.length;
	const hasValidationError =
		newCustomRowErrors.size > 0 || previewAppsValidationIssues.length > 0;
	const hasUnsavedChanges = changedCount > 0;
	const hasAnyConfig = configs.length > 0;
	const handleSaveShortcut = useEffectEvent((event: KeyboardEvent) => {
		const mod = event.metaKey || event.ctrlKey;
		if (!mod || event.key.toLowerCase() !== "s") {
			return;
		}

		event.preventDefault();
		if (event.repeat) {
			return;
		}

		void handleSaveAll();
	});

	useEffect(() => {
		if (loading || !hasAnyConfig || tabCategories.length === 0) {
			return;
		}

		if (section !== resolvedSection) {
			navigate(`/admin/settings/${resolvedSection}`, { replace: true });
		}
	}, [
		hasAnyConfig,
		loading,
		navigate,
		resolvedSection,
		section,
		tabCategories,
	]);

	const getCategoryLabel = useCallback(
		(category: string) => {
			switch (category) {
				case "user":
					return t("settings_category_user");
				case "auth":
					return t("settings_category_auth");
				case "network":
					return t("settings_category_network");
				case "operations":
					return t("settings_category_operations");
				case "mail":
					return t("settings_category_mail");
				case "storage":
					return t("settings_category_storage");
				case "webdav":
					return t("settings_category_webdav");
				case "audit":
					return t("settings_category_audit");
				case "general":
					return t("settings_category_general");
				case "custom":
					return t("settings_category_custom");
				case "other":
					return t("settings_category_other");
				default:
					return category;
			}
		},
		[t],
	);

	const getCategoryDescription = useCallback(
		(category: string) => {
			switch (category) {
				case "user":
					return t("settings_category_user_desc");
				case "auth":
					return t("settings_category_auth_desc");
				case "network":
					return t("settings_category_network_desc");
				case "operations":
					return t("settings_category_operations_desc");
				case "mail":
					return t("settings_category_mail_desc");
				case "storage":
					return t("settings_category_storage_desc");
				case "webdav":
					return t("settings_category_webdav_desc");
				case "audit":
					return t("settings_category_audit_desc");
				case "general":
					return t("settings_category_general_desc");
				case "custom":
					return t("custom_config_intro");
				case "other":
					return t("settings_category_other_desc");
				default:
					return undefined;
			}
		},
		[t],
	);

	const categorySummaries = useMemo<CategorySummary[]>(
		() =>
			tabCategories.map((category) => ({
				category,
				description: getCategoryDescription(category),
				icon: getCategoryIcon(category),
				label: getCategoryLabel(category),
			})),
		[getCategoryDescription, getCategoryLabel, tabCategories],
	);

	const categorySummaryMap = useMemo(
		() =>
			new Map(
				categorySummaries.map(
					(summary) => [summary.category, summary] as const,
				),
			),
		[categorySummaries],
	);

	const activeCategorySummary =
		categorySummaryMap.get(activeTab) ?? categorySummaries[0] ?? null;

	const compactOrderedSummaries = useMemo<CategorySummary[]>(
		() =>
			tabCategories.flatMap((category) => {
				const summary = categorySummaryMap.get(category);
				return summary ? [summary] : [];
			}),
		[categorySummaryMap, tabCategories],
	);

	const compactOverflowSummaries = useMemo<CategorySummary[]>(
		() =>
			compactOrderedSummaries.filter(
				(summary) => !compactInlineCategories.includes(summary.category),
			),
		[compactInlineCategories, compactOrderedSummaries],
	);

	const compactInlineSummaries = useMemo<CategorySummary[]>(
		() =>
			compactOrderedSummaries.filter((summary) =>
				compactInlineCategories.includes(summary.category),
			),
		[compactInlineCategories, compactOrderedSummaries],
	);

	const compactOverflowActiveSummary = useMemo(() => {
		return (
			compactOverflowSummaries.find(
				(summary) => summary.category === activeTab,
			) ?? null
		);
	}, [activeTab, compactOverflowSummaries]);

	const isMobileNavigation = viewportWidth < MOBILE_BREAKPOINT;
	const isDesktopNavigation = viewportWidth >= DESKTOP_NAV_BREAKPOINT;
	const isCompactNavigation = !isMobileNavigation && !isDesktopNavigation;

	useEffect(() => {
		if (activeTab === resolvedSection) {
			return;
		}

		setTabDirection(
			(CATEGORY_INDEX[resolvedSection] ?? Number.MAX_SAFE_INTEGER) >=
				(CATEGORY_INDEX[activeTab] ?? Number.MAX_SAFE_INTEGER)
				? "forward"
				: "backward",
		);
		setActiveTab(resolvedSection);
	}, [activeTab, resolvedSection]);

	useEffect(() => {
		document.addEventListener("keydown", handleSaveShortcut);
		return () => document.removeEventListener("keydown", handleSaveShortcut);
	}, []);

	useEffect(() => {
		saveBarPhaseRef.current = saveBarPhase;
	}, [saveBarPhase]);

	useEffect(() => {
		if (saveBarTimerRef.current !== null) {
			window.clearTimeout(saveBarTimerRef.current);
			saveBarTimerRef.current = null;
		}

		if (hasUnsavedChanges) {
			setSaveBarPhase("entering");
			saveBarTimerRef.current = window.setTimeout(() => {
				setSaveBarPhase("visible");
				saveBarTimerRef.current = null;
			}, SAVE_BAR_ENTER_DURATION_MS);
			return;
		}

		if (saveBarPhaseRef.current === "hidden") {
			return;
		}

		setSaveBarPhase((previous) =>
			previous === "hidden" ? previous : "exiting",
		);
		saveBarTimerRef.current = window.setTimeout(() => {
			setSaveBarPhase("hidden");
			saveBarTimerRef.current = null;
		}, SAVE_BAR_EXIT_DURATION_MS);

		return () => {
			if (saveBarTimerRef.current !== null) {
				window.clearTimeout(saveBarTimerRef.current);
				saveBarTimerRef.current = null;
			}
		};
	}, [hasUnsavedChanges]);

	useEffect(() => {
		return () => {
			if (saveBarTimerRef.current !== null) {
				window.clearTimeout(saveBarTimerRef.current);
			}
		};
	}, []);

	useEffect(() => {
		if (saveBarPhase === "hidden") {
			setSaveBarReservedHeight(0);
			return;
		}

		const fallbackHeight =
			viewportWidth < MOBILE_BREAKPOINT
				? ADMIN_SETTINGS_SAVE_BAR_MIN_RESERVED_HEIGHT_MOBILE_PX
				: ADMIN_SETTINGS_SAVE_BAR_MIN_RESERVED_HEIGHT_DESKTOP_PX;
		const node = saveBarMeasureRef.current;
		if (!node) {
			setSaveBarReservedHeight(fallbackHeight);
			return;
		}

		const updateReservedHeight = () => {
			const measuredHeight = Math.ceil(node.getBoundingClientRect().height);
			setSaveBarReservedHeight(Math.max(measuredHeight, fallbackHeight));
		};

		updateReservedHeight();

		if (typeof ResizeObserver === "undefined") {
			return;
		}

		const resizeObserver = new ResizeObserver(() => {
			updateReservedHeight();
		});
		resizeObserver.observe(node);

		return () => {
			resizeObserver.disconnect();
		};
	}, [saveBarPhase, viewportWidth]);

	useEffect(() => {
		if (!isCompactNavigation) {
			setCompactInlineCategories((previous) =>
				areCategoryListsEqual(previous, tabCategories)
					? previous
					: tabCategories,
			);
			return;
		}

		const measureCompactNavigation = () => {
			const containerWidth = compactNavContainerRef.current?.clientWidth ?? 0;
			if (tabCategories.length === 0) {
				setCompactInlineCategories([]);
				return;
			}

			const tabWidths = tabCategories.map(
				(category) => compactTabMeasureRefs.current[category]?.offsetWidth ?? 0,
			);

			if (containerWidth <= 0 || tabWidths.some((width) => width <= 0)) {
				setCompactInlineCategories((previous) =>
					areCategoryListsEqual(previous, tabCategories)
						? previous
						: tabCategories,
				);
				return;
			}

			const totalTabWidth =
				tabWidths.reduce((sum, width) => sum + width, 0) +
				Math.max(0, tabWidths.length - 1) * COMPACT_NAV_TAB_GAP;

			if (totalTabWidth <= containerWidth) {
				setCompactInlineCategories((previous) =>
					areCategoryListsEqual(previous, tabCategories)
						? previous
						: tabCategories,
				);
				return;
			}

			const overflowTriggerWidth = Math.max(
				compactOverflowDefaultMeasureRef.current?.offsetWidth ?? 0,
				...tabCategories.map(
					(category) =>
						compactOverflowMeasureRefs.current[category]?.offsetWidth ?? 0,
				),
			);
			const availableWidth = Math.max(
				0,
				containerWidth - overflowTriggerWidth - COMPACT_NAV_OVERFLOW_GAP,
			);

			let usedWidth = 0;
			const nextInlineCategories: string[] = [];

			for (let index = 0; index < tabCategories.length; index += 1) {
				const category = tabCategories[index];
				const width = tabWidths[index];
				const gap = nextInlineCategories.length > 0 ? COMPACT_NAV_TAB_GAP : 0;

				if (
					nextInlineCategories.length === 0 ||
					usedWidth + gap + width <= availableWidth
				) {
					nextInlineCategories.push(category);
					usedWidth += gap + width;
				}
			}

			setCompactInlineCategories((previous) =>
				areCategoryListsEqual(previous, nextInlineCategories)
					? previous
					: nextInlineCategories,
			);
		};

		measureCompactNavigation();

		if (
			typeof ResizeObserver === "undefined" ||
			!compactNavContainerRef.current
		) {
			return;
		}

		const resizeObserver = new ResizeObserver(() => {
			measureCompactNavigation();
		});
		resizeObserver.observe(compactNavContainerRef.current);

		return () => {
			resizeObserver.disconnect();
		};
	}, [isCompactNavigation, tabCategories]);

	const getDraftValue = useCallback(
		(config: SystemConfig) => draftValues[config.key] ?? config.value,
		[draftValues],
	);

	const handleCategoryChange = useCallback(
		(value: string) => {
			if (
				value === activeTab ||
				!CATEGORY_ORDER.includes(value as AdminSettingsTab)
			) {
				return;
			}

			setTabDirection(
				(CATEGORY_INDEX[value] ?? Number.MAX_SAFE_INTEGER) >=
					(CATEGORY_INDEX[activeTab] ?? Number.MAX_SAFE_INTEGER)
					? "forward"
					: "backward",
			);
			setActiveTab(value);
			navigate(`/admin/settings/${value}`, { viewTransition: true });
		},
		[activeTab, navigate],
	);

	const updateDraftValue = (key: string, value: string) => {
		setDraftValues((previous) => ({ ...previous, [key]: value }));
	};

	const toggleSubcategoryGroup = useCallback(
		(groupKey: string, nextExpanded: boolean) => {
			setExpandedSubcategoryGroups((previous) => ({
				...previous,
				[groupKey]: nextExpanded,
			}));
		},
		[],
	);

	const toggleTemplateGroup = useCallback(
		(groupKey: string, nextExpanded: boolean) => {
			setExpandedTemplateGroups((previous) => ({
				...previous,
				[groupKey]: nextExpanded,
			}));
		},
		[],
	);

	const discardChanges = () => {
		setDraftValues(buildDraftValues(configs));
		setDisplayUnits({});
		setDeletedCustomKeys([]);
		setNewCustomRows([]);
	};

	const addCustomDraftRow = () => {
		customDraftIdRef.current += 1;
		setNewCustomRows((previous) => [
			...previous,
			{
				id: `new-custom-${customDraftIdRef.current}`,
				key: "",
				value: "",
			},
		]);
		handleCategoryChange("custom");
	};

	const updateNewCustomRow = (
		id: string,
		field: keyof Omit<NewCustomDraft, "id">,
		value: string,
	) => {
		setNewCustomRows((previous) =>
			previous.map((row) => (row.id === id ? { ...row, [field]: value } : row)),
		);
	};

	const removeNewCustomRow = (id: string) => {
		setNewCustomRows((previous) => previous.filter((row) => row.id !== id));
	};

	const markCustomDeleted = (key: string) => {
		setDeletedCustomKeys((previous) =>
			previous.includes(key) ? previous : [...previous, key],
		);
	};

	const restoreDeletedCustom = (key: string) => {
		setDeletedCustomKeys((previous) => previous.filter((item) => item !== key));
	};

	const handleSaveAll = async () => {
		if (saving || hasValidationError || !hasUnsavedChanges) {
			return;
		}

		try {
			setSaving(true);
			const nextConfigsByKey = new Map(
				configs.map((config) => [config.key, config] as const),
			);

			for (const config of deletedCustomConfigs) {
				await adminConfigService.delete(config.key);
				nextConfigsByKey.delete(config.key);
			}

			for (const config of changedExistingConfigs) {
				const nextValue = getDraftValue(config);
				const savedConfig = await adminConfigService.set(config.key, nextValue);
				nextConfigsByKey.set(
					config.key,
					savedConfig.key === config.key
						? savedConfig
						: { ...config, value: nextValue },
				);
			}

			for (const row of activeNewCustomRows) {
				const key = row.key.trim();
				const savedConfig = await adminConfigService.set(key, row.value);
				if (savedConfig.key !== key) {
					throw new Error(`Saved config key mismatch: expected ${key}`);
				}
				nextConfigsByKey.set(key, savedConfig);
			}

			const nextConfigs = Array.from(nextConfigsByKey.values());
			setConfigs(nextConfigs);
			const nextPublicSiteUrl =
				nextConfigsByKey.get(PUBLIC_SITE_URL_KEY)?.value;
			if (nextPublicSiteUrl !== undefined) {
				syncPublicSiteUrlRuntime(nextPublicSiteUrl);
			}
			toast.success(t("settings_saved"));
		} catch (error) {
			handleApiError(error);
			try {
				await load({ showLoading: false });
			} catch (reloadError) {
				handleApiError(reloadError);
			}
		} finally {
			setSaving(false);
		}
	};

	const renderFieldMeta = (config: SystemConfig) => {
		const draftChanged = getDraftValue(config) !== config.value;
		const requiresRestart = getConfigRequiresRestart(config);
		const configLabel = getSystemConfigLabel(config);
		const configDescription = getSystemConfigDescription(config);
		const showRawKey = configLabel !== config.key;
		const showTemplateVariableLink =
			config.category === "mail.template" && config.key.endsWith("_html");

		return (
			<div className="space-y-1">
				<div className="flex flex-wrap items-center gap-2">
					<p
						className={
							showRawKey
								? "break-words text-sm font-medium"
								: "break-all font-mono text-sm font-medium"
						}
					>
						{configLabel}
					</p>
					{draftChanged ? (
						<span className="text-xs font-medium text-primary">
							{t("settings_status_unsaved")}
						</span>
					) : null}
					{requiresRestart ? (
						<span className="text-xs text-muted-foreground">
							{t("requires_restart")}
						</span>
					) : null}
				</div>
				{configDescription ? (
					<p className="max-w-3xl break-words text-sm text-muted-foreground">
						{configDescription}
					</p>
				) : null}
				{showTemplateVariableLink ? (
					<button
						type="button"
						className="w-fit text-sm text-primary underline-offset-4 transition-colors hover:text-primary/80 hover:underline"
						onClick={() => openTemplateVariablesDialog(config)}
					>
						{t("mail_template_variable_link")}
					</button>
				) : null}
			</div>
		);
	};

	const renderCategoryHeader = (
		category: string,
		options?: {
			description?: string;
			extra?: ReactNode;
		},
	) => {
		const description =
			options && "description" in options
				? options.description
				: getCategoryDescription(category);

		return (
			<div className="max-w-4xl space-y-3">
				<div className="space-y-1">
					<h3 className="text-xl font-semibold tracking-tight">
						{getCategoryLabel(category)}
					</h3>
					{description ? (
						<p className="max-w-3xl break-words text-sm leading-6 text-muted-foreground">
							{description}
						</p>
					) : null}
				</div>
				{options?.extra}
			</div>
		);
	};

	const getSubcategoryLabel = useCallback(
		(category: string, subcategory?: string) => {
			if (!subcategory) {
				return getCategoryLabel(category);
			}

			const translationKey = `settings_subcategory_${category}_${subcategory.replaceAll(".", "_")}`;
			const translated = t(translationKey);
			return translated === translationKey
				? formatSubcategoryLabel(subcategory)
				: translated;
		},
		[getCategoryLabel, t],
	);

	const getSubcategoryDescription = useCallback(
		(category: string, subcategory?: string) => {
			if (!subcategory) {
				return undefined;
			}

			const translationKey = `settings_subcategory_${category}_${subcategory.replaceAll(".", "_")}_desc`;
			const translated = t(translationKey);
			return translated === translationKey ? undefined : translated;
		},
		[t],
	);

	const getMailTemplateGroupLabel = useCallback(
		(groupId: string) => {
			const translationKey = `settings_mail_template_group_${groupId}`;
			const translated = t(translationKey);
			return translated === translationKey
				? formatSubcategoryLabel(groupId)
				: translated;
		},
		[t],
	);

	const renderScaledNumberInputControl = (
		config: SystemConfig,
		draftValue: string,
		units: readonly DisplayUnit[],
		unitLabelKey: string,
		options?: {
			fullWidth?: boolean;
		},
	) => {
		if (draftValue.trim() && parseWholeNumber(draftValue) === null) {
			return null;
		}

		const availableUnits = getAvailableDisplayUnits(units, draftValue);
		const preferredUnit = getPreferredDisplayUnit(units, draftValue);
		const selectedUnit =
			availableUnits.find((unit) => unit.value === displayUnits[config.key]) ??
			preferredUnit;

		return (
			<div
				className={cn(
					"flex flex-col gap-3 sm:flex-row sm:items-center",
					options?.fullWidth ? "w-full max-w-2xl" : "max-w-2xl",
				)}
			>
				<Input
					type="number"
					inputMode="numeric"
					step="1"
					className="w-full sm:max-w-48"
					value={formatDisplayValue(draftValue, selectedUnit)}
					onChange={(event) => {
						const nextDisplayValue = event.target.value.trim();
						if (!nextDisplayValue) {
							updateDraftValue(config.key, "");
							return;
						}
						if (!/^\d+$/.test(nextDisplayValue)) {
							return;
						}

						const parsed = Number(nextDisplayValue);
						if (!Number.isSafeInteger(parsed)) {
							return;
						}

						updateDraftValue(
							config.key,
							String(parsed * selectedUnit.multiplier),
						);
					}}
					placeholder={t("config_value")}
				/>
				<Select
					items={availableUnits.map((unit) => ({
						label: t(unit.labelKey),
						value: unit.value,
					}))}
					value={selectedUnit.value}
					onValueChange={(value) =>
						setDisplayUnits((previous) => ({
							...previous,
							[config.key]: value as
								| TimeDisplayUnitValue
								| SizeDisplayUnitValue,
						}))
					}
				>
					<SelectTrigger
						id={`${config.key}-unit`}
						width="fit"
						className="min-w-28"
						aria-label={t(unitLabelKey)}
					>
						<SelectValue />
					</SelectTrigger>
					<SelectContent>
						{availableUnits.map((unit) => (
							<SelectItem key={unit.value} value={unit.value}>
								{t(unit.labelKey)}
							</SelectItem>
						))}
					</SelectContent>
				</Select>
			</div>
		);
	};

	const renderTimeConfigInputControl = (
		config: SystemConfig,
		draftValue: string,
		options?: {
			fullWidth?: boolean;
		},
	) => {
		const baseUnit = getTimeConfigBaseUnit(config);
		if (!baseUnit) {
			return null;
		}

		return renderScaledNumberInputControl(
			config,
			draftValue,
			TIME_DISPLAY_UNITS[baseUnit],
			"settings_time_unit_label",
			options,
		);
	};

	const renderSizeConfigInputControl = (
		config: SystemConfig,
		draftValue: string,
		options?: {
			fullWidth?: boolean;
		},
	) => {
		if (!isSizeConfig(config)) {
			return null;
		}

		return renderScaledNumberInputControl(
			config,
			draftValue,
			SIZE_DISPLAY_UNITS,
			"settings_size_unit_label",
			options,
		);
	};

	const renderConfigInputControl = (
		config: SystemConfig,
		draftValue: string,
		options?: {
			fullWidth?: boolean;
		},
	) => {
		const valueType = getConfigValueType(config);
		const isSensitive = getConfigIsSensitive(config);
		const multiline = isMultilineType(valueType);
		const brandingPreviewAppearance = isBrandingAssetConfig(config)
			? getBrandingAssetPreviewAppearance(config)
			: null;

		if (brandingPreviewAppearance) {
			return (
				<div className="flex max-w-4xl items-end gap-3">
					<div className="w-full max-w-3xl">
						<Input
							type={
								isNumberType(valueType)
									? "number"
									: isSensitive
										? "password"
										: "text"
							}
							inputMode={isNumberType(valueType) ? "decimal" : "text"}
							value={draftValue}
							onChange={(event) =>
								updateDraftValue(config.key, event.target.value)
							}
							placeholder={t("config_value")}
						/>
					</div>
					<UrlAssetPreview
						url={draftValue}
						appearance={brandingPreviewAppearance}
					/>
				</div>
			);
		}

		if (config.key === PREVIEW_APPS_CONFIG_KEY) {
			return (
				<PreviewAppsConfigEditor
					onBuildWopiDiscoveryPreviewConfig={
						handleBuildWopiDiscoveryPreviewConfig
					}
					value={draftValue}
					onChange={(nextValue) => updateDraftValue(config.key, nextValue)}
				/>
			);
		}

		if (multiline) {
			return (
				<ConfigCodeEditor
					language={getConfigEditorLanguage(config)}
					theme={editorTheme}
					value={draftValue}
					onChange={(value) => updateDraftValue(config.key, value)}
				/>
			);
		}

		const timeConfigInput = renderTimeConfigInputControl(
			config,
			draftValue,
			options,
		);
		if (timeConfigInput) {
			return timeConfigInput;
		}

		const sizeConfigInput = renderSizeConfigInputControl(
			config,
			draftValue,
			options,
		);
		if (sizeConfigInput) {
			return sizeConfigInput;
		}

		return (
			<Input
				type={
					isNumberType(valueType) ? "number" : isSensitive ? "password" : "text"
				}
				inputMode={isNumberType(valueType) ? "decimal" : "text"}
				className={options?.fullWidth ? "w-full max-w-2xl" : "max-w-2xl"}
				value={draftValue}
				onChange={(event) => updateDraftValue(config.key, event.target.value)}
				placeholder={t("config_value")}
			/>
		);
	};

	const renderSystemConfigRow = (config: SystemConfig) => {
		const draftValue = getDraftValue(config);
		const valueType = getConfigValueType(config);
		return (
			<div className="space-y-3">
				{renderFieldMeta(config)}
				{valueType === "boolean" ? (
					<div className="flex items-center gap-3 text-sm">
						<Switch
							id={config.key}
							checked={draftValue === "true"}
							onCheckedChange={(checked) =>
								updateDraftValue(config.key, checked ? "true" : "false")
							}
						/>
						<span>
							{draftValue === "true"
								? t("settings_value_on")
								: t("settings_value_off")}
						</span>
					</div>
				) : (
					renderConfigInputControl(config, draftValue)
				)}
			</div>
		);
	};

	const renderCustomConfigRow = (config: SystemConfig) => {
		const draftValue = getDraftValue(config);
		const valueType = getConfigValueType(config);
		const draftChanged = draftValue !== config.value;
		const multiline = isMultilineType(valueType);

		return (
			<div className="space-y-3">
				<div className="space-y-1">
					<div className="flex flex-wrap items-center gap-2">
						<p className="break-all font-mono text-sm font-medium">
							{config.key}
						</p>
						{draftChanged ? (
							<span className="text-xs font-medium text-primary">
								{t("settings_status_unsaved")}
							</span>
						) : null}
					</div>
					{getConfigDescription(config) ? (
						<p className="max-w-3xl break-words text-sm text-muted-foreground">
							{getConfigDescription(config)}
						</p>
					) : null}
				</div>

				<div
					className={
						multiline
							? "space-y-3"
							: "flex flex-col gap-3 sm:flex-row sm:items-center"
					}
				>
					{renderConfigInputControl(config, draftValue, { fullWidth: true })}
					<Button
						variant="ghost"
						size="sm"
						className="justify-start text-destructive"
						onClick={() => markCustomDeleted(config.key)}
					>
						{t("core:delete")}
					</Button>
				</div>
			</div>
		);
	};

	const renderNewCustomRow = (row: NewCustomDraft) => {
		const error = newCustomRowErrors.get(row.id);

		return (
			<div className="space-y-3">
				<p className="text-sm font-medium text-muted-foreground">
					{t("custom_config_new_entry")}
				</p>
				<div className="flex max-w-4xl flex-col gap-3 lg:flex-row">
					<Input
						className="lg:max-w-sm"
						value={row.key}
						aria-invalid={error ? true : undefined}
						onChange={(event) =>
							updateNewCustomRow(row.id, "key", event.target.value)
						}
						placeholder={t("custom_config_key_placeholder")}
					/>
					<Input
						className="lg:max-w-xl"
						value={row.value}
						onChange={(event) =>
							updateNewCustomRow(row.id, "value", event.target.value)
						}
						placeholder={t("config_value")}
					/>
					<Button
						variant="ghost"
						size="sm"
						className="justify-start text-destructive"
						onClick={() => removeNewCustomRow(row.id)}
					>
						{t("core:delete")}
					</Button>
				</div>
				{error ? <p className="text-sm text-destructive">{error}</p> : null}
			</div>
		);
	};

	const renderCategoryContent = (category: string) => {
		const panelAnimationClass =
			tabDirection === "forward"
				? "animate-in fade-in duration-300 slide-in-from-right-4 motion-reduce:animate-none"
				: "animate-in fade-in duration-300 slide-in-from-left-4 motion-reduce:animate-none";
		const showCategoryHeader = !isMobileNavigation;
		const systemConfigGroups = systemSubcategoryGroups[category] ?? [];
		const hasSubcategorySections =
			systemConfigGroups.length > 1 ||
			systemConfigGroups.some((group) => group.subcategory);
		const customCategoryActions = (
			<div className="space-y-3">
				<p className="text-xs text-muted-foreground">
					{t("config_namespace_hint")}
				</p>
				<Button
					variant="ghost"
					size="sm"
					className="justify-start px-0"
					onClick={addCustomDraftRow}
				>
					<Icon name="Plus" className="h-4 w-4" />
					{t("add_custom_config")}
				</Button>
			</div>
		);
		const mailConfigActions = (
			<div className="flex flex-col items-start gap-2 lg:items-end">
				<Button variant="outline" size="sm" onClick={openTestEmailDialog}>
					<Icon name="EnvelopeSimple" className="h-4 w-4" />
					{t("mail_send_test_email")}
				</Button>
				<p className="max-w-xs text-xs text-muted-foreground lg:text-right">
					{t("mail_send_test_email_hint")}
				</p>
			</div>
		);
		const renderSystemConfigGroups = () => {
			if (!hasSubcategorySections) {
				return (
					<div className="max-w-4xl divide-y divide-border/40">
						{(systemGroups[category] ?? []).map((config) => (
							<div key={config.key} className="py-6 first:pt-0 last:pb-0">
								{renderSystemConfigRow(config)}
							</div>
						))}
					</div>
				);
			}

			return (
				<div className="max-w-5xl space-y-4">
					{systemConfigGroups.map((group) => {
						const groupKey = getSubcategoryGroupKey(
							category,
							group.subcategory,
						);
						const isMailTemplateSection =
							category === "mail" && group.subcategory === "template";
						const collapsible =
							!isMailTemplateSection &&
							group.configs.some((config) =>
								isMultilineType(getConfigValueType(config)),
							);
						const defaultExpanded = !collapsible;
						const expanded =
							expandedSubcategoryGroups[groupKey] ?? defaultExpanded;
						const groupDescription = getSubcategoryDescription(
							category,
							group.subcategory,
						);
						const extra =
							category === "mail" && group.subcategory === "config"
								? mailConfigActions
								: null;
						const mailTemplateGroups = isMailTemplateSection
							? Array.from(
									group.configs.reduce((map, config) => {
										const templateGroupId = getMailTemplateGroupId(config.key);
										const existingGroup = map.get(templateGroupId);
										if (existingGroup) {
											existingGroup.push(config);
											return map;
										}

										map.set(templateGroupId, [config]);
										return map;
									}, new Map<string, SystemConfig[]>()),
								)
									.sort(([left], [right]) => {
										return (
											getMailTemplateGroupOrderIndex(left) -
												getMailTemplateGroupOrderIndex(right) ||
											left.localeCompare(right)
										);
									})
									.map(([templateGroupId, configs]) => ({
										configs: [...configs].sort(
											(left, right) =>
												getMailTemplateFieldOrder(left.key) -
													getMailTemplateFieldOrder(right.key) ||
												left.key.localeCompare(right.key),
										),
										groupKey: `${groupKey}:${templateGroupId}`,
										templateGroupId,
									}))
							: [];

						return (
							<section
								key={groupKey}
								className="overflow-hidden rounded-2xl border border-border/60 bg-card/40"
							>
								<div className="flex flex-col gap-4 px-5 py-4 lg:flex-row lg:items-start lg:justify-between">
									<div className="min-w-0 flex-1 space-y-1">
										<h4 className="text-base font-semibold tracking-tight">
											{getSubcategoryLabel(category, group.subcategory)}
										</h4>
										{groupDescription ? (
											<p className="max-w-3xl break-words text-sm leading-6 text-muted-foreground">
												{groupDescription}
											</p>
										) : null}
									</div>
									<div className="flex flex-col items-start gap-3 lg:items-end">
										{extra}
										{collapsible ? (
											<Button
												variant="ghost"
												size="sm"
												className="justify-start px-0 lg:px-3"
												aria-expanded={expanded}
												onClick={() =>
													toggleSubcategoryGroup(groupKey, !expanded)
												}
											>
												{expanded
													? t("settings_section_collapse")
													: t("settings_section_expand")}
												<Icon
													name="CaretDown"
													className={cn(
														"h-4 w-4 transition-transform",
														expanded ? "rotate-180" : "",
													)}
												/>
											</Button>
										) : null}
									</div>
								</div>
								{!collapsible || expanded ? (
									<div className="border-t border-border/40 px-5">
										{isMailTemplateSection ? (
											<div className="space-y-4 py-5">
												{mailTemplateGroups.map((templateGroup) => {
													const templateExpanded =
														expandedTemplateGroups[templateGroup.groupKey] ??
														false;
													const changedCount = templateGroup.configs.filter(
														(config) => getDraftValue(config) !== config.value,
													).length;

													return (
														<section
															key={templateGroup.groupKey}
															className="overflow-hidden rounded-xl border border-border/50 bg-background/80"
														>
															<Button
																variant="ghost"
																size="sm"
																className="flex h-auto w-full items-center justify-between gap-3 rounded-none px-4 py-3 text-left"
																aria-expanded={templateExpanded}
																onClick={() =>
																	toggleTemplateGroup(
																		templateGroup.groupKey,
																		!templateExpanded,
																	)
																}
															>
																<span className="min-w-0 space-y-1">
																	<span className="block text-sm font-medium">
																		{getMailTemplateGroupLabel(
																			templateGroup.templateGroupId,
																		)}
																	</span>
																	{changedCount > 0 ? (
																		<span className="block text-xs text-primary">
																			{t("settings_save_notice", {
																				count: changedCount,
																			})}
																		</span>
																	) : null}
																</span>
																<span className="flex shrink-0 items-center gap-2 text-xs text-muted-foreground">
																	<span>
																		{templateExpanded
																			? t("settings_section_collapse")
																			: t("settings_section_expand")}
																	</span>
																	<Icon
																		name="CaretDown"
																		className={cn(
																			"h-4 w-4 transition-transform",
																			templateExpanded ? "rotate-180" : "",
																		)}
																	/>
																</span>
															</Button>
															<AnimatedCollapsible
																open={templateExpanded}
																contentClassName={cn(
																	"px-4 transition-colors duration-[180ms] ease-out motion-reduce:transition-none",
																	templateExpanded
																		? "border-t border-border/40"
																		: "border-t border-transparent",
																)}
															>
																<div className="divide-y divide-border/40">
																	{templateGroup.configs.map((config) => (
																		<div
																			key={config.key}
																			className="py-5 first:pt-5 last:pb-5"
																		>
																			{renderSystemConfigRow(config)}
																		</div>
																	))}
																</div>
															</AnimatedCollapsible>
														</section>
													);
												})}
											</div>
										) : (
											<div className="divide-y divide-border/40">
												{group.configs.map((config) => (
													<div
														key={config.key}
														className="py-6 first:pt-6 last:pb-6"
													>
														{renderSystemConfigRow(config)}
													</div>
												))}
											</div>
										)}
									</div>
								) : null}
							</section>
						);
					})}
				</div>
			);
		};

		if (category === "custom") {
			return (
				<div
					key={`${activeTab}-${tabDirection}`}
					className={`space-y-10 ${panelAnimationClass}`}
				>
					{showCategoryHeader
						? renderCategoryHeader(category, {
								description: undefined,
								extra: customCategoryActions,
							})
						: customCategoryActions}

					{visibleCustomConfigs.length === 0 &&
					newCustomRows.length === 0 &&
					deletedCustomConfigs.length === 0 ? (
						<p className="text-sm text-muted-foreground">
							{t("custom_config_empty")}
						</p>
					) : null}

					{visibleCustomConfigs.length > 0 ? (
						<div className="max-w-4xl divide-y divide-border/40">
							{visibleCustomConfigs.map((config) => (
								<div key={config.key} className="py-6 first:pt-0 last:pb-0">
									{renderCustomConfigRow(config)}
								</div>
							))}
						</div>
					) : null}

					{newCustomRows.length > 0 ? (
						<div className="max-w-4xl divide-y divide-border/40">
							{newCustomRows.map((row) => (
								<div key={row.id} className="py-6 first:pt-0 last:pb-0">
									{renderNewCustomRow(row)}
								</div>
							))}
						</div>
					) : null}

					{deletedCustomConfigs.length > 0 ? (
						<div className="max-w-4xl space-y-4">
							<p className="text-sm text-muted-foreground">
								{t("custom_config_delete_staged")}
							</p>
							<div className="divide-y divide-border/40">
								{deletedCustomConfigs.map((config) => (
									<div
										key={config.key}
										className="space-y-2 py-4 opacity-70 first:pt-0 last:pb-0"
									>
										<p className="break-all font-mono text-sm line-through">
											{config.key}
										</p>
										<Button
											variant="ghost"
											size="sm"
											className="justify-start px-0"
											onClick={() => restoreDeletedCustom(config.key)}
										>
											{t("restore")}
										</Button>
									</div>
								))}
							</div>
						</div>
					) : null}
				</div>
			);
		}

		if (category === "mail") {
			return (
				<div
					key={`${activeTab}-${tabDirection}`}
					className={`space-y-10 ${panelAnimationClass}`}
				>
					{showCategoryHeader
						? renderCategoryHeader(category, { description: undefined })
						: null}
					{renderSystemConfigGroups()}
				</div>
			);
		}

		return (
			<div
				key={`${activeTab}-${tabDirection}`}
				className={`space-y-10 ${panelAnimationClass}`}
			>
				{showCategoryHeader
					? renderCategoryHeader(category, { description: undefined })
					: null}
				{renderSystemConfigGroups()}
			</div>
		);
	};

	const renderCategoryMenuItems = (summaries: CategorySummary[]) => (
		<DropdownMenuRadioGroup
			value={activeTab}
			onValueChange={(value) => handleCategoryChange(value)}
		>
			{summaries.map((summary) => (
				<DropdownMenuRadioItem
					key={summary.category}
					value={summary.category}
					className="gap-2 pr-10"
				>
					<Icon name={summary.icon} className="h-4 w-4 text-muted-foreground" />
					<span className="min-w-0 flex-1 truncate">{summary.label}</span>
				</DropdownMenuRadioItem>
			))}
		</DropdownMenuRadioGroup>
	);

	const renderCompactNavigationTabContent = (
		summary: CategorySummary,
		className?: string,
	) => (
		<span
			className={
				className
					? `${COMPACT_NAV_TAB_CONTENT_CLASS} ${className}`
					: COMPACT_NAV_TAB_CONTENT_CLASS
			}
		>
			<Icon name={summary.icon} className="h-4 w-4" />
			<span>{summary.label}</span>
		</span>
	);

	const renderDesktopNavigation = () => (
		<div className="sticky top-0 self-start min-h-0 w-[17.5rem] flex-none border-r border-border/40 pr-4">
			<TabsList
				variant="line"
				className="h-auto w-full flex-col items-stretch gap-1 bg-transparent p-0"
			>
				{categorySummaries.map((summary) => (
					<TabsTrigger
						key={summary.category}
						value={summary.category}
						className="group h-auto w-full justify-start rounded-2xl border-0 px-0 py-0 text-left whitespace-normal text-foreground/75 after:hidden hover:text-foreground data-active:bg-transparent data-active:text-foreground"
					>
						<div className="flex w-full items-start gap-3 rounded-2xl px-4 py-3 transition-colors duration-200 group-hover:bg-muted/40 group-data-[active]:bg-muted/55">
							<div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-muted/80 text-muted-foreground transition-colors duration-200 group-data-[active]:bg-background group-data-[active]:text-foreground">
								<Icon name={summary.icon} className="h-4 w-4" />
							</div>
							<div className="min-w-0 flex-1 space-y-1">
								<p className="break-words text-sm font-medium leading-5">
									{summary.label}
								</p>
								{summary.description ? (
									<p className="break-words text-xs leading-5 text-muted-foreground">
										{summary.description}
									</p>
								) : null}
							</div>
						</div>
					</TabsTrigger>
				))}
			</TabsList>
		</div>
	);

	const renderCompactNavigation = () => (
		<div
			ref={compactNavContainerRef}
			className="flex flex-none items-end gap-3"
		>
			<TabsList
				variant="line"
				className="h-auto min-w-0 flex-1 justify-start gap-2 overflow-hidden border-b border-border/40 bg-transparent px-0 pb-2"
			>
				{compactInlineSummaries.map((summary) => (
					<TabsTrigger
						key={summary.category}
						value={summary.category}
						className={`${COMPACT_NAV_TAB_TRIGGER_CLASS} text-muted-foreground hover:text-foreground data-active:text-foreground`}
					>
						{renderCompactNavigationTabContent(summary)}
					</TabsTrigger>
				))}
			</TabsList>

			{compactOverflowSummaries.length > 0 ? (
				<DropdownMenu>
					<DropdownMenuTrigger
						render={
							<Button
								variant={compactOverflowActiveSummary ? "secondary" : "ghost"}
								size="sm"
								className="shrink-0 rounded-full px-3"
							>
								<Icon
									name={compactOverflowActiveSummary?.icon ?? "DotsThree"}
									className="h-4 w-4"
								/>
								<span>
									{compactOverflowActiveSummary?.label ?? t("core:more")}
								</span>
							</Button>
						}
					/>
					<DropdownMenuContent align="end" className="w-64 min-w-64">
						{renderCategoryMenuItems(compactOverflowSummaries)}
					</DropdownMenuContent>
				</DropdownMenu>
			) : null}
		</div>
	);

	const renderCompactNavigationMeasurements = () => {
		if (!isCompactNavigation) {
			return null;
		}

		return (
			<div
				aria-hidden="true"
				className="pointer-events-none fixed top-0 left-0 -z-10 flex items-center gap-2 opacity-0"
			>
				{compactOrderedSummaries.map((summary) => (
					<button
						key={`measure-tab-${summary.category}`}
						ref={(node) => {
							compactTabMeasureRefs.current[summary.category] = node;
						}}
						type="button"
						className={`${COMPACT_NAV_TAB_TRIGGER_CLASS} inline-flex items-center whitespace-nowrap`}
					>
						{renderCompactNavigationTabContent(summary)}
					</button>
				))}
				<button
					ref={compactOverflowDefaultMeasureRef}
					type="button"
					className={COMPACT_NAV_OVERFLOW_TRIGGER_CLASS}
				>
					<Icon name="DotsThree" className="h-4 w-4" />
					<span>{t("core:more")}</span>
				</button>
				{compactOrderedSummaries.map((summary) => (
					<button
						key={`measure-overflow-${summary.category}`}
						ref={(node) => {
							compactOverflowMeasureRefs.current[summary.category] = node;
						}}
						type="button"
						className={COMPACT_NAV_OVERFLOW_TRIGGER_CLASS}
					>
						<Icon name={summary.icon} className="h-4 w-4" />
						<span>{summary.label}</span>
					</button>
				))}
			</div>
		);
	};

	const renderMobileNavigation = () => {
		if (!activeCategorySummary) {
			return null;
		}

		return (
			<div className="flex-none space-y-2">
				<p className="text-xs font-medium tracking-[0.18em] text-muted-foreground uppercase">
					{t("config_category")}
				</p>
				<DropdownMenu>
					<DropdownMenuTrigger
						render={
							<Button
								variant="outline"
								className="h-auto w-full justify-between rounded-2xl px-4 py-3"
							>
								<span className="flex min-w-0 items-center gap-3">
									<span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-muted text-muted-foreground">
										<Icon
											name={activeCategorySummary.icon}
											className="h-4 w-4"
										/>
									</span>
									<span className="min-w-0 text-left">
										<span className="block truncate text-sm font-medium">
											{activeCategorySummary.label}
										</span>
										<span className="block text-xs text-muted-foreground">
											{t("config_category")}
										</span>
									</span>
								</span>
								<span className="ml-3 flex shrink-0 items-center gap-2">
									<Icon
										name="CaretDown"
										className="h-4 w-4 text-muted-foreground"
									/>
								</span>
							</Button>
						}
					/>
					<DropdownMenuContent
						align="start"
						className="w-[min(24rem,calc(100vw-2rem))] min-w-64"
					>
						{renderCategoryMenuItems(categorySummaries)}
					</DropdownMenuContent>
				</DropdownMenu>
			</div>
		);
	};

	const renderFloatingSaveBar = () => {
		if (saveBarPhase === "hidden") {
			return null;
		}

		return (
			<div
				ref={saveBarMeasureRef}
				data-testid="settings-save-bar"
				aria-hidden={!hasUnsavedChanges}
				className="pointer-events-none fixed right-0 bottom-0 left-0 z-30 px-4 pb-4 md:left-60 md:px-6 md:pb-6"
			>
				<div
					className={cn(
						"mx-auto w-full max-w-4xl origin-bottom will-change-transform motion-reduce:animate-none",
						saveBarPhase === "entering"
							? "pointer-events-auto animate-in fade-in zoom-in-95 slide-in-from-bottom-4 duration-[180ms] ease-out"
							: saveBarPhase === "visible"
								? "pointer-events-auto translate-y-0 opacity-100"
								: "pointer-events-none animate-out fade-out zoom-out-95 slide-out-to-bottom-4 duration-[140ms] ease-in",
					)}
				>
					<div
						className={cn(
							"rounded-2xl border bg-background/95 shadow-xl shadow-black/5 ring-1 backdrop-blur supports-[backdrop-filter]:bg-background/80",
							hasValidationError
								? "border-destructive/40 ring-destructive/10"
								: "border-border/70 ring-border/50",
						)}
					>
						<div className="flex flex-col gap-4 px-4 py-3 sm:flex-row sm:items-center sm:justify-between sm:px-5">
							<div className="min-w-0 flex-1 space-y-1">
								<p className="text-sm font-medium">
									{t("settings_save_notice", { count: changedCount })}
								</p>
								<p
									className={
										hasValidationError
											? "text-sm text-destructive"
											: "text-sm text-muted-foreground"
									}
								>
									{hasValidationError
										? previewAppsValidationIssues.length > 0
											? t("preview_apps_validation_error")
											: t("custom_config_validation_error")
										: t("settings_save_hint")}
								</p>
							</div>
							<div className="flex flex-wrap items-center gap-3 sm:justify-end">
								<Button
									variant="ghost"
									disabled={saving || !hasUnsavedChanges}
									onClick={discardChanges}
								>
									{t("undo_changes")}
								</Button>
								<Button
									className="w-fit"
									disabled={saving || hasValidationError || !hasUnsavedChanges}
									onClick={() => void handleSaveAll()}
								>
									{saving ? t("save_changes") : t("save_changes")}
								</Button>
							</div>
						</div>
					</div>
				</div>
			</div>
		);
	};

	const renderTemplateVariablesDialog = () => (
		<Dialog
			open={activeTemplateVariableGroupCode !== null}
			onOpenChange={(open) => {
				if (!open) {
					setActiveTemplateVariableGroupCode(null);
				}
			}}
		>
			<DialogContent className="max-w-[calc(100%-1.5rem)] sm:max-w-[min(72rem,calc(100vw-2rem))]">
				<DialogHeader>
					<DialogTitle>
						{t("mail_template_variables_dialog_title", {
							name: activeTemplateVariableGroup
								? getTemplateVariableGroupLabel(activeTemplateVariableGroup)
								: "",
						})}
					</DialogTitle>
					<DialogDescription>
						{t("mail_template_variables_dialog_desc")}
					</DialogDescription>
				</DialogHeader>
				<div className="max-h-[min(70vh,40rem)] space-y-4 overflow-y-auto py-2 pr-1">
					{activeTemplateVariableGroup ? (
						<div className="space-y-3">
							{activeTemplateVariableGroup.variables.map((variable) => (
								<div
									key={`${activeTemplateVariableGroup.template_code}:${variable.token}`}
									className="rounded-xl border border-border/60 bg-card/40 px-4 py-4"
								>
									<div className="flex flex-wrap items-center gap-2">
										<code className="break-all rounded bg-muted px-2 py-1 font-mono text-xs">
											{variable.token}
										</code>
										<span className="text-sm font-medium">
											{getTemplateVariableLabel(variable)}
										</span>
									</div>
									{getTemplateVariableDescription(variable) ? (
										<p className="mt-2 break-words text-sm leading-6 text-muted-foreground">
											{getTemplateVariableDescription(variable)}
										</p>
									) : null}
								</div>
							))}
						</div>
					) : (
						<p className="text-sm text-muted-foreground">
							{t("mail_template_variables_dialog_empty")}
						</p>
					)}
				</div>
				<DialogFooter showCloseButton />
			</DialogContent>
		</Dialog>
	);

	return (
		<AdminLayout>
			<AdminPageShell className="pb-0 md:pb-0">
				<AdminPageHeader
					title={t("system_settings")}
					description={t("settings_intro")}
				/>

				{loading ? (
					<SkeletonTable columns={4} rows={8} />
				) : !hasAnyConfig ? (
					<EmptyState title={t("no_config")} />
				) : (
					<div
						data-testid="settings-content"
						className="flex flex-col gap-8 transition-[padding-bottom] duration-200 ease-out"
						style={{
							paddingBottom: `${settingsContentBaseBottomPadding + saveBarReservedHeight}px`,
						}}
					>
						<Tabs
							orientation={isDesktopNavigation ? "vertical" : "horizontal"}
							value={activeTab}
							onValueChange={handleCategoryChange}
							className={
								isDesktopNavigation
									? "flex min-w-0 flex-row items-start gap-10"
									: "flex min-w-0 flex-col gap-6"
							}
						>
							{isMobileNavigation ? renderMobileNavigation() : null}
							{isCompactNavigation ? renderCompactNavigation() : null}
							{isDesktopNavigation ? renderDesktopNavigation() : null}
							{renderCompactNavigationMeasurements()}

							<div
								className={
									isDesktopNavigation ? "min-w-0 flex-1 pr-2" : "min-w-0 pr-2"
								}
							>
								{tabCategories.map((category) => (
									<TabsContent
										key={category}
										value={category}
										className="min-w-0 pt-0"
									>
										{renderCategoryContent(category)}
									</TabsContent>
								))}
							</div>
						</Tabs>
					</div>
				)}
			</AdminPageShell>
			{renderTemplateVariablesDialog()}
			<Dialog
				open={testEmailDialogOpen}
				onOpenChange={(open) => {
					if (!sendingTestEmail) {
						setTestEmailDialogOpen(open);
					}
				}}
			>
				<DialogContent className="max-w-md">
					<DialogHeader>
						<DialogTitle>{t("mail_test_email_dialog_title")}</DialogTitle>
						<DialogDescription>
							{t("mail_test_email_dialog_desc")}
						</DialogDescription>
					</DialogHeader>
					<div className="space-y-2 py-2">
						<p className="text-sm font-medium">
							{t("mail_test_email_recipient_label")}
						</p>
						<Input
							type="email"
							value={testEmailTarget}
							onChange={(event) => setTestEmailTarget(event.target.value)}
							placeholder={t("mail_test_email_recipient_placeholder")}
						/>
					</div>
					<DialogFooter>
						<Button
							variant="outline"
							disabled={sendingTestEmail}
							onClick={() => setTestEmailDialogOpen(false)}
						>
							{t("core:cancel")}
						</Button>
						<Button
							disabled={sendingTestEmail}
							onClick={() => void handleSendTestEmail()}
						>
							{t("mail_send_test_email")}
						</Button>
					</DialogFooter>
				</DialogContent>
			</Dialog>
			{renderFloatingSaveBar()}
		</AdminLayout>
	);
}
