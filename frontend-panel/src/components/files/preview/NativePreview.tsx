import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { EmptyState } from "@/components/common/EmptyState";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import type { NativePreviewSession } from "@/types/api";
import {
	EmbeddedWebAppPreview,
	TRUSTED_DOCUMENT_VIEWER_IFRAME_ALLOW,
	TRUSTED_DOCUMENT_VIEWER_IFRAME_SANDBOX,
} from "./EmbeddedWebAppPreview";
import { PreviewLoadingState } from "./PreviewLoadingState";

interface NativePreviewProps {
	createSession: () => Promise<NativePreviewSession>;
	label: string;
}

interface NativePreviewState {
	requestKey: NativePreviewRequestKey;
	session: NativePreviewSession | null;
}

interface NativePreviewRequestKey {
	createSession: NativePreviewProps["createSession"];
	label: string;
}

function requestNativePreviewSession(requestKey: NativePreviewRequestKey) {
	return requestKey.createSession();
}

export function NativePreview({ createSession, label }: NativePreviewProps) {
	const { t } = useTranslation("files");
	const [previewState, setPreviewState] = useState<NativePreviewState | null>(
		null,
	);
	const requestKey = useMemo<NativePreviewRequestKey>(
		() => ({ createSession, label }),
		[createSession, label],
	);

	useEffect(() => {
		let cancelled = false;

		void requestNativePreviewSession(requestKey)
			.then((session) => {
				if (cancelled) return;
				setPreviewState({ requestKey, session });
			})
			.catch(() => {
				if (cancelled) return;
				setPreviewState({ requestKey, session: null });
			});

		return () => {
			cancelled = true;
		};
	}, [requestKey]);

	const isLoading = previewState?.requestKey !== requestKey;
	const session = isLoading ? null : previewState.session;

	const openTarget = () => {
		if (!session) return;
		window.open(session.action_url, "_blank", "noopener,noreferrer");
	};

	if (isLoading) {
		return (
			<PreviewLoadingState
				text={t("native_preview_loading", { label })}
				className="h-full min-h-[16rem]"
			/>
		);
	}

	if (!session) {
		return (
			<EmptyState
				icon={<Icon name="Globe" className="size-10" />}
				title={t("native_preview_unavailable")}
				description={t("native_preview_unavailable_desc")}
			/>
		);
	}

	if (session.mode === "new_tab") {
		return (
			<EmptyState
				icon={<Icon name="ArrowSquareOut" className="size-10" />}
				title={label}
				description={t("native_preview_external_desc", { label })}
				action={
					<Button variant="outline" onClick={openTarget}>
						<Icon name="ArrowSquareOut" className="mr-2 size-4" />
						{t("native_preview_open", { label })}
					</Button>
				}
			/>
		);
	}

	return (
		<EmbeddedWebAppPreview
			title={label}
			src={session.action_url}
			iframeAllow={TRUSTED_DOCUMENT_VIEWER_IFRAME_ALLOW}
			iframeReferrerPolicy="no-referrer"
			iframeSandbox={TRUSTED_DOCUMENT_VIEWER_IFRAME_SANDBOX}
			actions={
				<Button variant="outline" size="sm" onClick={openTarget}>
					<Icon name="ArrowSquareOut" className="mr-2 size-4" />
					{t("native_preview_open", { label })}
				</Button>
			}
		/>
	);
}
