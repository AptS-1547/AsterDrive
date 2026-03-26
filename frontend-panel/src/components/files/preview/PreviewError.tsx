import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";

interface PreviewErrorProps {
	onRetry?: () => void;
}

export function PreviewError({ onRetry }: PreviewErrorProps) {
	const { t } = useTranslation("files");
	return (
		<div className="flex flex-col items-center justify-center gap-3 p-8 text-center">
			<Icon name="Warning" className="h-10 w-10 text-muted-foreground" />
			<p className="text-sm text-muted-foreground">
				{t("preview_load_failed")}
			</p>
			{onRetry && (
				<Button variant="outline" size="sm" onClick={onRetry}>
					<Icon name="ArrowCounterClockwise" className="mr-2 h-4 w-4" />
					{t("preview_retry")}
				</Button>
			)}
		</div>
	);
}
