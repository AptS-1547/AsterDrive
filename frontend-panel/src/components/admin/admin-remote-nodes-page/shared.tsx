import type { TFunction } from "i18next";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import { formatDateTime } from "@/lib/format";
import type { RemoteNodeInfo } from "@/types/api";

export const INTERACTIVE_TABLE_ROW_CLASS =
	"cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50";
export const REMOTE_NODE_TEXT_CELL_CONTENT_CLASS =
	"flex min-w-0 items-center rounded-lg bg-muted/10 px-3 py-3 text-left transition-colors duration-200";
export const REMOTE_NODE_BADGE_CELL_CONTENT_CLASS =
	"flex items-center rounded-lg bg-muted/20 px-3 py-3 text-left transition-colors duration-200";

export function TestConnectionButton({
	disabled = false,
	onTest,
}: {
	disabled?: boolean;
	onTest: () => Promise<boolean>;
}) {
	const { t } = useTranslation("admin");
	const [testing, setTesting] = useState(false);
	const [result, setResult] = useState<boolean | null>(null);

	const handleTest = async () => {
		setTesting(true);
		setResult(null);
		const passed = await onTest();
		setResult(passed);
		setTesting(false);
	};

	return (
		<Button
			type="button"
			variant="outline"
			className={ADMIN_CONTROL_HEIGHT_CLASS}
			disabled={disabled || testing}
			onClick={handleTest}
		>
			{testing ? (
				<Icon name="Spinner" className="mr-1 h-4 w-4 animate-spin" />
			) : result === true ? (
				<Icon name="Check" className="mr-1 h-4 w-4 text-green-600" />
			) : (
				<Icon name="WifiHigh" className="mr-1 h-4 w-4" />
			)}
			{t("test_connection")}
		</Button>
	);
}

export function getRemoteNodeStatusTone(node: RemoteNodeInfo) {
	if (!node.is_enabled) {
		return "border-slate-500/40 bg-slate-500/10 text-slate-600 dark:text-slate-300";
	}

	if (!node.last_checked_at) {
		return "border-blue-500/60 bg-blue-500/10 text-blue-600 dark:text-blue-300";
	}

	if (node.last_error) {
		return "border-amber-500/60 bg-amber-500/10 text-amber-600 dark:text-amber-300";
	}

	return "border-emerald-500/60 bg-emerald-500/10 text-emerald-600 dark:text-emerald-300";
}

export function getRemoteNodeStatusLabel(t: TFunction, node: RemoteNodeInfo) {
	if (!node.is_enabled) {
		return t("remote_node_status_disabled");
	}

	if (!node.last_checked_at) {
		return t("remote_node_status_pending");
	}

	if (node.last_error) {
		return t("remote_node_status_degraded");
	}

	return t("remote_node_status_enabled");
}

export function formatLastChecked(
	t: TFunction,
	lastCheckedAt: string | null | undefined,
) {
	return lastCheckedAt
		? formatDateTime(lastCheckedAt)
		: t("remote_node_never_checked");
}
