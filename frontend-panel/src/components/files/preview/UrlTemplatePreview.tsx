import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { EmptyState } from "@/components/common/EmptyState";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import type { PreviewLinkInfo } from "@/types/api";
import {
	EmbeddedWebAppPreview,
	EXTERNAL_WEB_APP_IFRAME_SANDBOX,
	EXTERNAL_WEB_APP_SAME_ORIGIN_IFRAME_SANDBOX,
} from "./EmbeddedWebAppPreview";
import { PreviewLoadingState } from "./PreviewLoadingState";
import {
	type ResolvedVideoBrowserTarget,
	resolveUrlTemplateTarget,
	type VideoBrowserFileContext,
} from "./video-browser-config";

interface UrlTemplatePreviewProps {
	createPreviewLink?: () => Promise<PreviewLinkInfo>;
	downloadPath: string;
	file: VideoBrowserFileContext;
	label: string;
	optionKey?: string;
	rawConfig: Record<string, unknown> | null | undefined;
}

const SAME_ORIGIN_SANDBOX_URL_TEMPLATE_KEYS = new Set([
	"builtin.office_google",
	"builtin.office_microsoft",
]);

interface UrlTemplatePreviewState {
	isLoading: boolean;
	target: ResolvedVideoBrowserTarget | null;
}

export function UrlTemplatePreview({
	createPreviewLink,
	downloadPath,
	file,
	label,
	optionKey,
	rawConfig,
}: UrlTemplatePreviewProps) {
	const { t } = useTranslation("files");
	const [{ isLoading, target }, setPreviewState] =
		useState<UrlTemplatePreviewState>({
			isLoading: true,
			target: null,
		});

	useEffect(() => {
		let cancelled = false;

		setPreviewState({ isLoading: true, target: null });

		void resolveUrlTemplateTarget(
			file,
			downloadPath,
			label,
			rawConfig,
			createPreviewLink,
		)
			.then((resolvedTarget) => {
				if (cancelled) return;
				setPreviewState({ isLoading: false, target: resolvedTarget });
			})
			.catch(() => {
				if (cancelled) return;
				setPreviewState({ isLoading: false, target: null });
			});

		return () => {
			cancelled = true;
		};
	}, [createPreviewLink, downloadPath, file, label, rawConfig]);

	const openTarget = () => {
		if (!target) return;
		window.open(target.url, "_blank", "noopener,noreferrer");
	};

	if (isLoading) {
		return (
			<PreviewLoadingState
				text={t("loading_preview")}
				className="h-full min-h-[16rem]"
			/>
		);
	}

	if (!target) {
		return (
			<EmptyState
				icon={<Icon name="Globe" className="size-10" />}
				title={t("url_template_unavailable")}
				description={t("url_template_unavailable_desc")}
			/>
		);
	}

	if (target.mode === "new_tab") {
		return (
			<EmptyState
				icon={<Icon name="ArrowSquareOut" className="size-10" />}
				title={target.label}
				description={t("url_template_external_desc", { label: target.label })}
				action={
					<Button variant="outline" onClick={openTarget}>
						<Icon name="ArrowSquareOut" className="mr-2 size-4" />
						{t("url_template_open", { label: target.label })}
					</Button>
				}
			/>
		);
	}

	return (
		<EmbeddedWebAppPreview
			title={target.label}
			src={target.url}
			actions={
				<Button variant="outline" size="sm" onClick={openTarget}>
					<Icon name="ArrowSquareOut" className="mr-2 size-4" />
					{t("url_template_open", { label: target.label })}
				</Button>
			}
			iframeAllow="autoplay; fullscreen; picture-in-picture"
			iframeReferrerPolicy="same-origin"
			iframeSandbox={
				optionKey && SAME_ORIGIN_SANDBOX_URL_TEMPLATE_KEYS.has(optionKey)
					? EXTERNAL_WEB_APP_SAME_ORIGIN_IFRAME_SANDBOX
					: EXTERNAL_WEB_APP_IFRAME_SANDBOX
			}
		/>
	);
}
