import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { SettingsPageIntro } from "@/components/common/SettingsScaffold";
import { AppLayout } from "@/components/layout/AppLayout";
import { InterfaceSettingsView } from "@/components/settings/InterfaceSettingsView";
import { ProfileSettingsView } from "@/components/settings/ProfileSettingsView";
import { SecuritySettingsView } from "@/components/settings/SecuritySettingsView";
import { TeamsSettingsView } from "@/components/settings/TeamsSettingsView";
import { Icon } from "@/components/ui/icon";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { usePageTitle } from "@/hooks/usePageTitle";
import { SETTINGS_PAGE_CONTENT_PADDING_CLASS } from "@/lib/constants";

type SettingsTabKey = "profile" | "interface" | "security" | "teams";

function getSettingsSectionTitle(
	section: SettingsTabKey,
	t: ReturnType<typeof useTranslation>["t"],
) {
	switch (section) {
		case "interface":
			return t("settings:settings_interface");
		case "security":
			return t("settings:settings_security");
		case "teams":
			return t("settings:settings_teams");
		default:
			return t("settings:settings_profile");
	}
}

export default function SettingsPage({
	section = "profile",
}: {
	section?: SettingsTabKey;
}) {
	const { t } = useTranslation(["core", "settings"]);
	const navigate = useNavigate();
	usePageTitle(getSettingsSectionTitle(section, t));

	const handleSectionChange = (value: string) => {
		if (
			(value === "profile" ||
				value === "interface" ||
				value === "security" ||
				value === "teams") &&
			value !== section
		) {
			navigate(`/settings/${value}`, { viewTransition: true });
		}
	};

	return (
		<AppLayout>
			<div className="min-h-0 flex-1 overflow-auto">
				<div
					className={`mx-auto flex w-full max-w-6xl flex-col gap-5 ${SETTINGS_PAGE_CONTENT_PADDING_CLASS}`}
				>
					<SettingsPageIntro
						title={t("settings")}
						description={t("settings:settings_page_desc")}
					/>

					<Tabs
						value={section}
						onValueChange={handleSectionChange}
						className="flex flex-col gap-4"
					>
						<TabsList
							variant="line"
							className="h-auto w-full gap-5 border-b px-0 pb-2"
						>
							<TabsTrigger
								value="profile"
								className="h-10 min-w-0 rounded-none px-0"
							>
								<Icon name="PencilSimple" className="h-4 w-4" />
								<span className="truncate">
									{t("settings:settings_profile")}
								</span>
							</TabsTrigger>
							<TabsTrigger
								value="interface"
								className="h-10 min-w-0 rounded-none px-0"
							>
								<Icon name="Monitor" className="h-4 w-4" />
								<span className="truncate">
									{t("settings:settings_interface")}
								</span>
							</TabsTrigger>
							<TabsTrigger
								value="security"
								className="h-10 min-w-0 rounded-none px-0"
							>
								<Icon name="Shield" className="h-4 w-4" />
								<span className="truncate">
									{t("settings:settings_security")}
								</span>
							</TabsTrigger>
							<TabsTrigger
								value="teams"
								className="h-10 min-w-0 rounded-none px-0"
							>
								<Icon name="Cloud" className="h-4 w-4" />
								<span className="truncate">{t("settings:settings_teams")}</span>
							</TabsTrigger>
						</TabsList>

						<TabsContent value="profile" className="outline-none">
							<ProfileSettingsView />
						</TabsContent>

						<TabsContent value="interface" className="outline-none">
							<InterfaceSettingsView />
						</TabsContent>

						<TabsContent value="security" className="outline-none">
							<SecuritySettingsView />
						</TabsContent>

						<TabsContent value="teams" className="outline-none">
							<TeamsSettingsView />
						</TabsContent>
					</Tabs>
				</div>
			</div>
		</AppLayout>
	);
}
