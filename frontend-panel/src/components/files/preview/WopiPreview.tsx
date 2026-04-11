import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { EmptyState } from "@/components/common/EmptyState";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import type { PreviewOpenMode, WopiLaunchSession } from "@/types/api";
import { EmbeddedWebAppPreview } from "./EmbeddedWebAppPreview";
import { PreviewLoadingState } from "./PreviewLoadingState";

interface WopiPreviewProps {
	createSession: () => Promise<WopiLaunchSession>;
	label: string;
	rawConfig: Record<string, unknown> | null | undefined;
}

function normalizeWopiMode(
	rawConfig: Record<string, unknown> | null | undefined,
	session: WopiLaunchSession | null,
): PreviewOpenMode {
	if (session?.mode === "new_tab") {
		return "new_tab";
	}
	if (session?.mode === "iframe") {
		return "iframe";
	}
	return rawConfig?.mode === "new_tab" ? "new_tab" : "iframe";
}

function buildWopiFormFields(
	session: WopiLaunchSession,
): Record<string, string> {
	return {
		access_token: session.access_token,
		access_token_ttl: String(session.access_token_ttl),
		...(session.form_fields ?? {}),
	};
}

function isValidActionUrl(value: string) {
	try {
		const parsed = new URL(value);
		return parsed.protocol === "http:" || parsed.protocol === "https:";
	} catch {
		return false;
	}
}

export function WopiPreview({
	createSession,
	label,
	rawConfig,
}: WopiPreviewProps) {
	const { t } = useTranslation("files");
	const iframeNameRef = useRef(
		`wopi-preview-${Math.random().toString(36).slice(2, 10)}`,
	);
	const formRef = useRef<HTMLFormElement | null>(null);
	const submittedKeyRef = useRef<string | null>(null);
	const [isLoading, setIsLoading] = useState(true);
	const [isFrameLoaded, setIsFrameLoaded] = useState(false);
	const [session, setSession] = useState<WopiLaunchSession | null>(null);

	useEffect(() => {
		let cancelled = false;

		setIsLoading(true);
		setIsFrameLoaded(false);
		setSession(null);
		submittedKeyRef.current = null;

		void createSession()
			.then((nextSession) => {
				if (cancelled) {
					return;
				}
				if (
					!nextSession.action_url ||
					!isValidActionUrl(nextSession.action_url)
				) {
					setSession(null);
					return;
				}
				setSession(nextSession);
			})
			.catch(() => {
				if (cancelled) {
					return;
				}
				setSession(null);
			})
			.finally(() => {
				if (cancelled) {
					return;
				}
				setIsLoading(false);
			});

		return () => {
			cancelled = true;
		};
	}, [createSession]);

	const mode = useMemo(
		() => normalizeWopiMode(rawConfig, session),
		[rawConfig, session],
	);
	const formFields = useMemo<Record<string, string>>(
		() => (session ? buildWopiFormFields(session) : {}),
		[session],
	);

	const submitToTarget = useCallback(
		(target: string) => {
			const form = formRef.current;
			if (!form || !session) {
				return false;
			}

			form.target = target;
			form.submit();
			return true;
		},
		[session],
	);

	useEffect(() => {
		if (!session || mode === "new_tab") {
			return;
		}

		const submissionKey = `${session.action_url}\u0000${session.access_token}\u0000${mode}`;
		if (submittedKeyRef.current === submissionKey) {
			return;
		}
		submittedKeyRef.current = submissionKey;

		const frameTarget = iframeNameRef.current;
		const timer = window.requestAnimationFrame(() => {
			submitToTarget(frameTarget);
		});

		return () => {
			window.cancelAnimationFrame(timer);
		};
	}, [mode, session, submitToTarget]);

	const openExternally = useCallback(() => {
		const target = `wopi-external-${Math.random().toString(36).slice(2, 10)}`;
		window.open("", target, "noopener,noreferrer");
		submitToTarget(target);
	}, [submitToTarget]);

	if (isLoading) {
		return (
			<PreviewLoadingState
				text={t("wopi_loading", { label })}
				className="h-full min-h-[16rem]"
			/>
		);
	}

	if (!session) {
		return (
			<EmptyState
				icon={<Icon name="Globe" className="h-10 w-10" />}
				title={t("wopi_unavailable")}
				description={t("wopi_unavailable_desc")}
			/>
		);
	}

	const form = (
		<form
			ref={formRef}
			action={session.action_url}
			method="post"
			className="hidden"
		>
			{Object.entries(formFields).map(([key, value]) => (
				<input key={key} type="hidden" name={key} value={value} />
			))}
		</form>
	);

	if (mode === "new_tab") {
		return (
			<>
				{form}
				<EmptyState
					icon={<Icon name="ArrowSquareOut" className="h-10 w-10" />}
					title={label}
					description={t("wopi_external_desc", { label })}
					action={
						<Button variant="outline" onClick={openExternally}>
							<Icon name="ArrowSquareOut" className="mr-2 h-4 w-4" />
							{t("wopi_open", { label })}
						</Button>
					}
				/>
			</>
		);
	}

	return (
		<>
			{form}
			<EmbeddedWebAppPreview
				title={label}
				src="about:blank"
				iframeName={iframeNameRef.current}
				onLoad={() => {
					if (submittedKeyRef.current === null) {
						return;
					}
					setIsFrameLoaded(true);
				}}
				iframeHidden={!isFrameLoaded}
				iframeReferrerPolicy="no-referrer"
				actions={
					<Button variant="outline" size="sm" onClick={openExternally}>
						<Icon name="ArrowSquareOut" className="mr-2 h-4 w-4" />
						{t("wopi_open", { label })}
					</Button>
				}
				loadingOverlay={
					!isFrameLoaded ? (
						<PreviewLoadingState
							text={t("wopi_loading", { label })}
							className="h-full min-h-0"
						/>
					) : undefined
				}
			/>
		</>
	);
}
