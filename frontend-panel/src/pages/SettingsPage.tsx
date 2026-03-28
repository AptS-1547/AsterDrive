import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { SettingsPageIntro } from "@/components/common/SettingsScaffold";
import { AppLayout } from "@/components/layout/AppLayout";
import { InterfaceSettingsView } from "@/components/settings/InterfaceSettingsView";
import { ProfileSettingsView } from "@/components/settings/ProfileSettingsView";
import { Icon } from "@/components/ui/icon";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

type SettingsTabKey = "profile" | "interface";

export default function SettingsPage({
	section = "profile",
}: {
	section?: SettingsTabKey;
}) {
	const { t } = useTranslation(["core", "settings"]);
	const navigate = useNavigate();

	const handleSectionChange = (value: string) => {
		if ((value === "profile" || value === "interface") && value !== section) {
			navigate(`/settings/${value}`, { viewTransition: true });
		}
	};

	return (
		<AppLayout>
			<div className="min-h-0 flex-1 overflow-auto">
				<div className="mx-auto flex w-full max-w-6xl flex-col gap-5 p-4 md:p-6">
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
							className="w-full justify-start gap-5 overflow-x-auto border-b px-0"
						>
							<TabsTrigger
								value="profile"
								className="h-10 flex-none rounded-none px-0"
							>
								<Icon name="PencilSimple" className="h-4 w-4" />
								<span>{t("settings:settings_profile")}</span>
							</TabsTrigger>
							<TabsTrigger
								value="interface"
								className="h-10 flex-none rounded-none px-0"
							>
								<Icon name="Monitor" className="h-4 w-4" />
								<span>{t("settings:settings_interface")}</span>
							</TabsTrigger>
						</TabsList>

						<TabsContent value="profile" className="outline-none">
							<ProfileSettingsView />
						</TabsContent>

						<TabsContent value="interface" className="outline-none">
							<InterfaceSettingsView />
						</TabsContent>
					</Tabs>
				</div>
			</div>
		</AppLayout>
	);
}
