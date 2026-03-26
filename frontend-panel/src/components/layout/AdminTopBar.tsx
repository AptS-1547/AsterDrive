import { useTranslation } from "react-i18next";
import { HeaderControls } from "@/components/layout/HeaderControls";
import { TopBarShell } from "@/components/layout/TopBarShell";
import { ADMIN_TOPBAR_HEIGHT_CLASS } from "@/lib/constants";

interface AdminTopBarProps {
	onSidebarToggle: () => void;
}

export function AdminTopBar({ onSidebarToggle }: AdminTopBarProps) {
	const { t } = useTranslation();

	return (
		<TopBarShell
			onSidebarToggle={onSidebarToggle}
			left={
				<div className="flex min-w-0 items-center gap-3">
					<img
						src="/static/logo.svg"
						alt={t("app_name")}
						className="hidden h-10 w-auto shrink-0 md:block"
					/>
					<h1 className="truncate text-base font-semibold sm:text-lg">
						{t("admin_panel")}
					</h1>
				</div>
			}
			right={<HeaderControls showHomeButton />}
			heightClassName={ADMIN_TOPBAR_HEIGHT_CLASS}
		/>
	);
}
