import { useTranslation } from "react-i18next";
import { TopBarShell } from "@/components/layout/TopBarShell";

export function ShareTopBar() {
	const { t } = useTranslation();

	return (
		<TopBarShell
			left={
				<img
					src="/static/asterdrive/logo.svg"
					alt={t("app_name")}
					className="h-10 w-auto shrink-0"
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
