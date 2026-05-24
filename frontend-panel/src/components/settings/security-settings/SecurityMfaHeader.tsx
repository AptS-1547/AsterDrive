import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";

interface SecurityMfaHeaderProps {
	enabled: boolean;
	loading: boolean;
	onRefresh: () => void;
}

export function SecurityMfaHeader({
	enabled,
	loading,
	onRefresh,
}: SecurityMfaHeaderProps) {
	const { t } = useTranslation(["core", "settings"]);

	return (
		<div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
			<div className="space-y-1">
				<div className="flex flex-wrap items-center gap-2">
					<h3 className="text-sm font-semibold">
						{t("settings:settings_mfa_section")}
					</h3>
					<Badge variant={enabled ? "default" : "secondary"}>
						{enabled
							? t("settings:settings_mfa_enabled_badge")
							: t("settings:settings_mfa_disabled_badge")}
					</Badge>
				</div>
				<p className="text-sm text-muted-foreground">
					{t("settings:settings_mfa_section_desc")}
				</p>
			</div>
			<Button
				type="button"
				variant="outline"
				disabled={loading}
				onClick={onRefresh}
			>
				{loading ? (
					<Icon name="Spinner" className="mr-2 size-4 animate-spin" />
				) : (
					<Icon name="ArrowClockwise" className="mr-2 size-4" />
				)}
				{t("core:refresh")}
			</Button>
		</div>
	);
}
