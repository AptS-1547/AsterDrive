import {
	type ReactNode,
	useCallback,
	useEffect,
	useEffectEvent,
	useMemo,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { AdminPageHeader } from "@/components/layout/AdminPageHeader";
import { AdminPageShell } from "@/components/layout/AdminPageShell";
import { Button, buttonVariants } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuRadioGroup,
	DropdownMenuRadioItem,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Icon, type IconName } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { handleApiError } from "@/hooks/useApiError";
import { cn } from "@/lib/utils";
import { adminConfigService } from "@/services/adminService";
import type { ConfigSchemaItem, SystemConfig } from "@/types/api";

const CATEGORY_ORDER = [
	"general",
	"auth",
	"network",
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
const COMPACT_NAV_TAB_TRIGGER_CLASS =
	"h-10 flex-none rounded-none px-0 text-sm font-medium";
const COMPACT_NAV_TAB_CONTENT_CLASS =
	"inline-flex items-center gap-2 px-3 py-2";
const COMPACT_NAV_OVERFLOW_TRIGGER_CLASS = buttonVariants({
	variant: "secondary",
	size: "sm",
	className: "shrink-0 rounded-full px-3",
});

const MASKED_VALUE = "********";

type DraftValues = Record<string, string>;

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

export type AdminSettingsTab = (typeof CATEGORY_ORDER)[number];
type SaveBarPhase = "hidden" | "entering" | "visible" | "exiting";

function getCategoryIcon(category: string): IconName {
	switch (category) {
		case "auth":
			return "Shield";
		case "network":
			return "Globe";
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

function isBrandingFaviconConfig(config: SystemConfig) {
	return config.key === "branding_favicon_url";
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

function UrlAssetPreview({ url }: { url: string }) {
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
		"group flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded-xl border bg-muted/20 transition-colors",
		normalizedUrl
			? "border-border/70 hover:border-primary/40 hover:bg-muted/30"
			: isInvalid
				? "border-amber-300/70 bg-amber-50/70"
				: "border-border/70",
	);

	const previewContent = normalizedUrl ? (
		<UrlAssetPreviewImage
			key={normalizedUrl}
			data-testid="branding-asset-preview-image"
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
					title={debouncedUrl.trim() || "/favicon.svg"}
					aria-label={debouncedUrl.trim() || "/favicon.svg"}
				>
					{previewContent}
				</div>
			)}
		</div>
	);
}

function UrlAssetPreviewImage({
	url,
	...props
}: {
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
			className="max-h-8 max-w-8 object-contain"
			onError={() => setHasLoadError(true)}
		/>
	);
}

function normalizeCategory(category?: string) {
	return category || "other";
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
	const navigate = useNavigate();
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
	const [configs, setConfigs] = useState<SystemConfig[]>([]);
	const [schemas, setSchemas] = useState<ConfigSchemaItem[]>([]);
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
	const [deletedCustomKeys, setDeletedCustomKeys] = useState<string[]>([]);
	const [newCustomRows, setNewCustomRows] = useState<NewCustomDraft[]>([]);
	const [compactInlineCategories, setCompactInlineCategories] = useState<
		string[]
	>([]);
	const [saveBarPhase, setSaveBarPhase] = useState<SaveBarPhase>("hidden");

	const load = useCallback(async (options?: { showLoading?: boolean }) => {
		const showLoading = options?.showLoading ?? true;

		try {
			if (showLoading) {
				setLoading(true);
			}
			const [cfgs, schemaList] = await Promise.all([
				adminConfigService.list({ limit: 200, offset: 0 }),
				adminConfigService.schema(),
			]);
			setConfigs(cfgs.items);
			setSchemas(schemaList);
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

	const changedCount =
		changedExistingConfigs.length +
		deletedCustomConfigs.length +
		activeNewCustomRows.length;
	const hasValidationError = newCustomRowErrors.size > 0;
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
				case "auth":
					return t("settings_category_auth");
				case "network":
					return t("settings_category_network");
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
				case "auth":
					return t("settings_category_auth_desc");
				case "network":
					return t("settings_category_network_desc");
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

	const formatDisplayValue = useCallback(
		(value: string, isSensitive: boolean) => {
			if (isSensitive) {
				return MASKED_VALUE;
			}

			return value.length > 0 ? value : t("settings_value_empty");
		},
		[t],
	);

	const getDefaultDisplayValue = useCallback(
		(config: SystemConfig) => {
			const schema = schemaMap.get(config.key);
			if (!schema) {
				return null;
			}

			return formatDisplayValue(schema.default_value, false);
		},
		[formatDisplayValue, schemaMap],
	);

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

	const discardChanges = () => {
		setDraftValues(buildDraftValues(configs));
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

			setConfigs(Array.from(nextConfigsByKey.values()));
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
		const defaultValue = getDefaultDisplayValue(config);
		const draftChanged = getDraftValue(config) !== config.value;
		const requiresRestart = getConfigRequiresRestart(config);
		const configLabel = getSystemConfigLabel(config);
		const configDescription = getSystemConfigDescription(config);
		const showRawKey = configLabel !== config.key;

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
				{defaultValue ? (
					<p className="break-words text-xs text-muted-foreground">
						{t("settings_default_value", { value: defaultValue })}
					</p>
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

	const renderSystemConfigRow = (config: SystemConfig) => {
		const draftValue = getDraftValue(config);
		const valueType = getConfigValueType(config);
		const isSensitive = getConfigIsSensitive(config);

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
				) : isBrandingFaviconConfig(config) ? (
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
						<UrlAssetPreview url={draftValue} />
					</div>
				) : (
					<Input
						type={
							isNumberType(valueType)
								? "number"
								: isSensitive
									? "password"
									: "text"
						}
						inputMode={isNumberType(valueType) ? "decimal" : "text"}
						className="max-w-2xl"
						value={draftValue}
						onChange={(event) =>
							updateDraftValue(config.key, event.target.value)
						}
						placeholder={t("config_value")}
					/>
				)}
			</div>
		);
	};

	const renderCustomConfigRow = (config: SystemConfig) => {
		const draftValue = getDraftValue(config);
		const valueType = getConfigValueType(config);
		const isSensitive = getConfigIsSensitive(config);
		const draftChanged = draftValue !== config.value;

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

				<div className="flex flex-col gap-3 sm:flex-row sm:items-center">
					<Input
						type={
							isNumberType(valueType)
								? "number"
								: isSensitive
									? "password"
									: "text"
						}
						inputMode={isNumberType(valueType) ? "decimal" : "text"}
						className="max-w-2xl"
						value={draftValue}
						onChange={(event) =>
							updateDraftValue(config.key, event.target.value)
						}
						placeholder={t("config_value")}
					/>
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

		return (
			<div
				key={`${activeTab}-${tabDirection}`}
				className={`space-y-10 ${panelAnimationClass}`}
			>
				{showCategoryHeader
					? renderCategoryHeader(category, { description: undefined })
					: null}
				<div className="max-w-4xl divide-y divide-border/40">
					{(systemGroups[category] ?? []).map((config) => (
						<div key={config.key} className="py-6 first:pt-0 last:pb-0">
							{renderSystemConfigRow(config)}
						</div>
					))}
				</div>
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
		<div className="min-h-0 w-[17.5rem] flex-none overflow-y-auto border-r border-border/40 pr-4">
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
										? t("custom_config_validation_error")
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

	return (
		<AdminLayout>
			<AdminPageShell>
				<AdminPageHeader
					title={t("system_settings")}
					description={t("settings_intro")}
				/>

				{loading ? (
					<SkeletonTable columns={4} rows={8} />
				) : !hasAnyConfig ? (
					<EmptyState title={t("no_config")} />
				) : (
					<div className="flex flex-col gap-8">
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
			{renderFloatingSaveBar()}
		</AdminLayout>
	);
}
