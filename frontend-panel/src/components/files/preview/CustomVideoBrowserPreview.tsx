import { useTranslation } from "react-i18next";
import { EmptyState } from "@/components/common/EmptyState";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import type { ResolvedVideoBrowserTarget } from "./video-browser-config";

interface CustomVideoBrowserPreviewProps {
	target: ResolvedVideoBrowserTarget | null;
}

export function CustomVideoBrowserPreview({
	target,
}: CustomVideoBrowserPreviewProps) {
	const { t } = useTranslation("files");

	const openTarget = () => {
		if (!target) return;
		window.open(target.url, "_blank", "noopener,noreferrer");
	};

	if (!target) {
		return (
			<EmptyState
				icon={<Icon name="Globe" className="h-10 w-10" />}
				title={t("video_browser_unavailable")}
				description={t("video_browser_unavailable_desc")}
			/>
		);
	}

	if (target.mode === "new_tab") {
		return (
			<EmptyState
				icon={<Icon name="ArrowSquareOut" className="h-10 w-10" />}
				title={target.label}
				description={t("video_browser_external_desc", { label: target.label })}
				action={
					<Button variant="outline" onClick={openTarget}>
						<Icon name="ArrowSquareOut" className="mr-2 h-4 w-4" />
						{t("video_browser_open", { label: target.label })}
					</Button>
				}
			/>
		);
	}

	return (
		<div className="flex h-full min-h-[70vh] flex-col gap-3">
			<div className="flex items-center justify-end">
				<Button variant="outline" size="sm" onClick={openTarget}>
					<Icon name="ArrowSquareOut" className="mr-2 h-4 w-4" />
					{t("video_browser_open", { label: target.label })}
				</Button>
			</div>
			<iframe
				title={target.label}
				src={target.url}
				className="min-h-0 flex-1 rounded-xl border bg-background"
				allow="autoplay; fullscreen; picture-in-picture"
				referrerPolicy="same-origin"
			/>
		</div>
	);
}
