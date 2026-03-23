import { useTranslation } from "react-i18next";
import { EmptyState } from "@/components/common/EmptyState";
import { Icon } from "@/components/ui/icon";

export function PreviewUnavailable() {
	const { t } = useTranslation("files");

	return (
		<EmptyState
			icon={<Icon name="EyeSlash" className="h-10 w-10" />}
			title={t("preview_not_available")}
			description={t("preview_not_available_desc")}
		/>
	);
}
