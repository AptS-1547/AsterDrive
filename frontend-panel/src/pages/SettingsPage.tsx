import { useTranslation } from "react-i18next";
import { ColorPresetPicker } from "@/components/common/ColorPresetPicker";
import {
	SettingsChoiceGroup,
	SettingsPageIntro,
	SettingsRow,
	SettingsSection,
} from "@/components/common/SettingsScaffold";
import { AppLayout } from "@/components/layout/AppLayout";
import type { IconName } from "@/components/ui/icon";
import { queuePreferenceSync } from "@/lib/preferenceSync";
import { useFileStore } from "@/stores/fileStore";
import { useThemeStore } from "@/stores/themeStore";

type ThemeMode = "light" | "dark" | "system";
type ViewMode = "list" | "grid";

export default function SettingsPage() {
	const { t, i18n } = useTranslation(["common", "files"]);
	const { mode, setMode } = useThemeStore();
	const viewMode = useFileStore((s) => s.viewMode);
	const setViewMode = useFileStore((s) => s.setViewMode);

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
	const currentLanguage = i18n.language?.startsWith("zh") ? "zh" : "en";
	const themeDescriptions: Record<ThemeMode, string> = {
		light: t("settings_theme_light_desc"),
		dark: t("settings_theme_dark_desc"),
		system: t("settings_theme_system_desc"),
	};
	const languageDescriptions = {
		en: t("settings_language_en_desc"),
		zh: t("settings_language_zh_desc"),
	};
	const browserDescriptions: Record<ViewMode, string> = {
		list: t("settings_browser_list_desc"),
		grid: t("settings_browser_grid_desc"),
	};

	return (
		<AppLayout>
			<div className="min-h-0 flex-1 overflow-auto">
				<div className="mx-auto flex w-full max-w-4xl flex-col gap-4 p-4 md:p-6">
					<SettingsPageIntro
						title={t("settings")}
						description={t("settings_page_desc")}
					/>

					<div className="space-y-4">
						<SettingsSection
							title={t("theme")}
							description={t("appearance_desc")}
						>
							<SettingsRow
								label={t("mode")}
								description={themeDescriptions[mode]}
							>
								<SettingsChoiceGroup
									options={themeOptions}
									value={mode}
									onChange={setMode}
								/>
							</SettingsRow>
							<SettingsRow
								label={t("color")}
								description={t("settings_color_desc")}
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
						</SettingsSection>

						<SettingsSection
							title={t("file_browser")}
							description={t("file_browser_desc")}
						>
							<SettingsRow
								label={t("mode")}
								description={browserDescriptions[viewMode]}
							>
								<SettingsChoiceGroup
									options={browserOptions}
									value={viewMode}
									onChange={setViewMode}
								/>
							</SettingsRow>
						</SettingsSection>
					</div>
				</div>
			</div>
		</AppLayout>
	);
}
