import { useTranslation } from "react-i18next";
import { ColorPresetPicker } from "@/components/common/ColorPresetPicker";
import {
	SettingsChoiceGroup,
	SettingsRow,
	SettingsSection,
} from "@/components/common/SettingsScaffold";
import type { IconName } from "@/components/ui/icon";
import { Switch } from "@/components/ui/switch";
import { queuePreferenceSync } from "@/lib/preferenceSync";
import { useAuthStore } from "@/stores/authStore";
import { type BrowserOpenMode, useFileStore } from "@/stores/fileStore";
import { useThemeStore } from "@/stores/themeStore";

type ThemeMode = "light" | "dark" | "system";
type ViewMode = "list" | "grid";

export function InterfaceSettingsView() {
	const { t, i18n } = useTranslation(["core", "files", "settings"]);
	const { mode, setMode } = useThemeStore();
	const viewMode = useFileStore((s) => s.viewMode);
	const browserOpenMode = useFileStore((s) => s.browserOpenMode);
	const setViewMode = useFileStore((s) => s.setViewMode);
	const setBrowserOpenMode = useFileStore((s) => s.setBrowserOpenMode);
	const storageEventStreamEnabled = useAuthStore(
		(s) => s.user?.preferences?.storage_event_stream_enabled !== false,
	);
	const setStorageEventStreamEnabled = useAuthStore(
		(s) => s.setStorageEventStreamEnabled,
	);

	const themeOptions: Array<{
		value: ThemeMode;
		label: string;
		icon: IconName;
	}> = [
		{ value: "light", label: t("theme_light"), icon: "Sun" },
		{ value: "dark", label: t("theme_dark"), icon: "Moon" },
		{ value: "system", label: t("theme_system"), icon: "Monitor" },
	];
	const languageOptions: Array<{
		value: "en" | "zh";
		label: string;
		icon: IconName;
	}> = [
		{ value: "en", label: t("language_en"), icon: "Globe" },
		{ value: "zh", label: t("language_zh"), icon: "Globe" },
	];
	const browserOptions: Array<{
		value: ViewMode;
		label: string;
		icon: IconName;
	}> = [
		{ value: "list", label: t("files:list_view"), icon: "ListBullets" },
		{ value: "grid", label: t("files:grid_view"), icon: "Grid" },
	];
	const browserOpenOptions: Array<{
		value: BrowserOpenMode;
		label: string;
	}> = [
		{
			value: "single_click",
			label: t("settings:settings_browser_open_single_click"),
		},
		{
			value: "double_click",
			label: t("settings:settings_browser_open_double_click"),
		},
	];

	const currentLanguage = i18n.language?.startsWith("zh") ? "zh" : "en";
	const themeDescriptions: Record<ThemeMode, string> = {
		light: t("settings:settings_theme_light_desc"),
		dark: t("settings:settings_theme_dark_desc"),
		system: t("settings:settings_theme_system_desc"),
	};
	const languageDescriptions = {
		en: t("settings:settings_language_en_desc"),
		zh: t("settings:settings_language_zh_desc"),
	};
	const browserDescriptions: Record<ViewMode, string> = {
		list: t("settings:settings_browser_list_desc"),
		grid: t("settings:settings_browser_grid_desc"),
	};
	const browserOpenDescriptions: Record<BrowserOpenMode, string> = {
		single_click: t("settings:settings_browser_open_single_click_desc"),
		double_click: t("settings:settings_browser_open_double_click_desc"),
	};
	const storageEventStreamDescription = storageEventStreamEnabled
		? t("settings:settings_storage_event_stream_enabled_desc")
		: t("settings:settings_storage_event_stream_disabled_desc");

	return (
		<SettingsSection
			title={t("settings:settings_interface")}
			description={t("settings:settings_interface_desc")}
			contentClassName="pt-2"
		>
			<SettingsRow label={t("theme")} description={themeDescriptions[mode]}>
				<SettingsChoiceGroup
					options={themeOptions}
					value={mode}
					onChange={setMode}
				/>
			</SettingsRow>

			<SettingsRow
				label={t("color")}
				description={t("settings:settings_color_desc")}
			>
				<div className="rounded-xl border bg-muted/20 p-3">
					<ColorPresetPicker />
				</div>
			</SettingsRow>

			<SettingsRow
				label={t("language")}
				description={languageDescriptions[currentLanguage]}
			>
				<SettingsChoiceGroup
					options={languageOptions}
					value={currentLanguage}
					onChange={(value) => {
						void i18n.changeLanguage(value);
						queuePreferenceSync({ language: value });
					}}
				/>
			</SettingsRow>

			<SettingsRow
				label={t("file_browser")}
				description={browserDescriptions[viewMode]}
			>
				<SettingsChoiceGroup
					options={browserOptions}
					value={viewMode}
					onChange={setViewMode}
				/>
			</SettingsRow>

			<SettingsRow
				label={t("settings:settings_browser_open_mode")}
				description={browserOpenDescriptions[browserOpenMode]}
			>
				<SettingsChoiceGroup
					options={browserOpenOptions}
					value={browserOpenMode}
					onChange={setBrowserOpenMode}
				/>
			</SettingsRow>

			<SettingsRow
				label={t("settings:settings_storage_event_stream")}
				description={storageEventStreamDescription}
				controlClassName="flex md:justify-end"
			>
				<div className="flex items-center justify-start md:justify-end">
					<Switch
						aria-label={t("settings:settings_storage_event_stream")}
						checked={storageEventStreamEnabled}
						onCheckedChange={(enabled) => {
							setStorageEventStreamEnabled(enabled);
							queuePreferenceSync({
								storage_event_stream_enabled: enabled,
							});
						}}
					/>
				</div>
			</SettingsRow>
		</SettingsSection>
	);
}
