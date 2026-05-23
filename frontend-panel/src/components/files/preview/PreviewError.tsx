import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";

interface PreviewErrorProps {
	messageKey?: string;
	onRetry?: () => void;
}

export function PreviewError({
	messageKey = "preview_load_failed",
	onRetry,
}: PreviewErrorProps) {
	const { t } = useTranslation("files");
	return (
		<div className="flex flex-col items-center justify-center gap-3 p-8 text-center">
			<Icon name="Warning" className="size-10 text-muted-foreground" />
			<p className="text-sm text-muted-foreground">{t(messageKey)}</p>
			{onRetry && (
				<Button variant="outline" size="sm" onClick={onRetry}>
					<Icon name="ArrowCounterClockwise" className="mr-2 size-4" />
					{t("preview_retry")}
				</Button>
			)}
		</div>
	);
}
