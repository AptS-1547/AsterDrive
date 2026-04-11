import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/lib/utils";

interface EmbeddedWebAppPreviewProps {
	actions?: ReactNode;
	errorOverlay?: ReactNode;
	headerStart?: ReactNode;
	iframeAllow?: string;
	iframeClassName?: string;
	iframeHidden?: boolean;
	iframeName?: string;
	iframeReferrerPolicy?: ComponentProps<"iframe">["referrerPolicy"];
	loadingOverlay?: ReactNode;
	onLoad?: () => void;
	src: string | null;
	title: string;
}

export function EmbeddedWebAppPreview({
	actions,
	errorOverlay,
	headerStart,
	iframeAllow,
	iframeClassName,
	iframeHidden = false,
	iframeName,
	iframeReferrerPolicy = "same-origin",
	loadingOverlay,
	onLoad,
	src,
	title,
}: EmbeddedWebAppPreviewProps) {
	return (
		<div className="flex h-full min-h-[70vh] flex-col gap-3">
			{headerStart || actions ? (
				<div className="flex flex-wrap items-center gap-2">
					{headerStart}
					{actions ? (
						<div
							className={cn(
								"flex flex-wrap items-center gap-2",
								headerStart ? "ml-auto" : "w-full justify-end",
							)}
						>
							{actions}
						</div>
					) : null}
				</div>
			) : null}
			<div className="relative min-h-0 flex-1 overflow-hidden rounded-xl border bg-background">
				{src ? (
					<iframe
						key={src}
						title={title}
						src={src}
						name={iframeName}
						className={cn(
							"h-full w-full bg-background",
							iframeHidden && "pointer-events-none opacity-0",
							iframeClassName,
						)}
						allow={iframeAllow}
						referrerPolicy={iframeReferrerPolicy}
						onLoad={onLoad}
					/>
				) : null}
				{loadingOverlay ? (
					<div className="absolute inset-0">{loadingOverlay}</div>
				) : null}
				{errorOverlay ? (
					<div className="absolute inset-0 flex items-center justify-center bg-background p-6">
						{errorOverlay}
					</div>
				) : null}
			</div>
		</div>
	);
}
