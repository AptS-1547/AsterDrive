import { useTranslation } from "react-i18next";
import { AsterDriveWordmark } from "@/components/common/AsterDriveWordmark";
import { TopBarShell } from "@/components/layout/TopBarShell";

export function ShareTopBar() {
	const { t } = useTranslation();

	return (
		<TopBarShell
			left={
				<AsterDriveWordmark
					alt={t("app_name")}
					className="h-16 w-auto shrink-0 px-6"
				/>
			}
			right={
				<span className="text-sm text-muted-foreground">
					{t("files:share")}
				</span>
			}
		/>
	);
}
