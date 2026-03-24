import { useTranslation } from "react-i18next";
import { ColorPresetPicker } from "@/components/common/ColorPresetPicker";
import { AppLayout } from "@/components/layout/AppLayout";
import {
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import { Icon, type IconName } from "@/components/ui/icon";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import { useFileStore } from "@/stores/fileStore";
import { useThemeStore } from "@/stores/themeStore";

type ThemeMode = "light" | "dark" | "system";
type ViewMode = "list" | "grid";

function OptionTile({
	icon,
	label,
	active,
	onClick,
}: {
	icon: IconName;
	label: string;
	active: boolean;
	onClick: () => void;
}) {
	return (
		<button
			type="button"
			onClick={onClick}
			className={cn(
				"flex w-full items-center gap-3 rounded-2xl border px-4 py-3 text-left transition-colors",
				active
					? "border-primary/30 bg-primary/8 shadow-sm"
					: "border-border bg-background hover:bg-muted/40",
			)}
		>
			<div
				className={cn(
					"flex h-9 w-9 items-center justify-center rounded-xl border",
					active
						? "border-primary/20 bg-primary/10 text-primary"
						: "border-border bg-muted text-muted-foreground",
				)}
			>
				<Icon name={icon} className="h-4 w-4" />
			</div>
			<span className="min-w-0 flex-1 font-medium">{label}</span>
			{active ? <Icon name="Check" className="h-4 w-4 text-primary" /> : null}
		</button>
	);
}

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

	const languageOptions = [
		{ value: "en", label: t("language_en"), icon: "Globe" as const },
		{ value: "zh", label: t("language_zh"), icon: "Globe" as const },
	];

	const browserOptions: Array<{
		value: ViewMode;
		label: string;
		icon: IconName;
	}> = [
		{ value: "list", label: t("files:list_view"), icon: "ListBullets" },
		{ value: "grid", label: t("files:grid_view"), icon: "Grid" },
	];

	return (
		<AppLayout>
			<div className="min-h-0 flex-1 overflow-auto">
				<div className="mx-auto flex w-full max-w-5xl flex-col gap-4 p-4 md:p-6">
					<div className="flex items-start gap-3 rounded-2xl border bg-muted/20 px-4 py-4">
						<div className="flex h-10 w-10 items-center justify-center rounded-2xl border border-primary/20 bg-primary/10 text-primary">
							<Icon name="Gear" className="h-4 w-4" />
						</div>
						<div className="min-w-0">
							<h1 className="text-xl font-semibold tracking-tight">
								{t("settings")}
							</h1>
							<p className="mt-1 text-sm text-muted-foreground">
								{t("settings_page_desc")}
							</p>
						</div>
					</div>

					<div className="grid gap-4 lg:grid-cols-[1.15fr_0.85fr]">
						<Card className="border-primary/10">
							<CardHeader className="border-b">
								<CardTitle>{t("theme")}</CardTitle>
								<CardDescription>{t("appearance_desc")}</CardDescription>
							</CardHeader>
							<CardContent className="space-y-5 pt-5">
								<div className="grid gap-3 sm:grid-cols-3">
									{themeOptions.map((option) => (
										<OptionTile
											key={option.value}
											icon={option.icon}
											label={option.label}
											active={mode === option.value}
											onClick={() => setMode(option.value)}
										/>
									))}
								</div>

								<Separator />

								<div className="space-y-3">
									<div>
										<p className="text-sm font-medium">{t("color")}</p>
										<p className="mt-1 text-sm text-muted-foreground">
											{t("settings_color_desc")}
										</p>
									</div>
									<div className="rounded-2xl border bg-muted/20 p-4">
										<ColorPresetPicker />
									</div>
								</div>
							</CardContent>
						</Card>

						<div className="space-y-4">
							<Card>
								<CardHeader className="border-b">
									<CardTitle>{t("language")}</CardTitle>
									<CardDescription>
										{t("settings_language_desc")}
									</CardDescription>
								</CardHeader>
								<CardContent className="space-y-3 pt-5">
									{languageOptions.map((option) => (
										<OptionTile
											key={option.value}
											icon={option.icon}
											label={option.label}
											active={Boolean(i18n.language?.startsWith(option.value))}
											onClick={() => i18n.changeLanguage(option.value)}
										/>
									))}
								</CardContent>
							</Card>

							<Card>
								<CardHeader className="border-b">
									<CardTitle>{t("file_browser")}</CardTitle>
									<CardDescription>{t("file_browser_desc")}</CardDescription>
								</CardHeader>
								<CardContent className="space-y-3 pt-5">
									{browserOptions.map((option) => (
										<OptionTile
											key={option.value}
											icon={option.icon}
											label={option.label}
											active={viewMode === option.value}
											onClick={() => setViewMode(option.value)}
										/>
									))}
								</CardContent>
							</Card>
						</div>
					</div>
				</div>
			</div>
		</AppLayout>
	);
}
