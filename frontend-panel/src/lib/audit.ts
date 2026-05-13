import type { TFunction } from "i18next";
import type { AuditAction } from "@/types/api";

export const AUDIT_ENTITY_TYPE_FILTER_VALUES = [
	"file",
	"folder",
	"team",
	"user",
	"share",
	"task",
	"resource_lock",
	"storage_policy",
	"policy_group",
	"system_config",
	"remote_node",
	"remote_ingress_profile",
	"webdav_account",
	"upload_session",
	"stream_ticket",
	"auth_session",
	"trash",
] as const;

function resolveAuditTranslation(
	t: TFunction,
	key: string,
	ns: "admin" | "settings",
	fallback?: string,
) {
	const translated = t(key, { ns, defaultValue: key });
	return translated === key ? fallback : translated;
}

export function formatAuditAction(t: TFunction, action: AuditAction | string) {
	const value = String(action);
	return (
		resolveAuditTranslation(t, `audit_action_${value}`, "admin") ??
		resolveAuditTranslation(t, value, "settings", value) ??
		value
	);
}

export function getAuditActionBadgeClass(action: AuditAction | string) {
	const value = String(action);
	if (value.includes("delete")) {
		return "border-red-200 bg-red-50 text-red-700 dark:border-red-900 dark:bg-red-950/60 dark:text-red-300";
	}
	if (value.includes("upload")) {
		return "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950/60 dark:text-emerald-300";
	}
	if (value.includes("share")) {
		return "border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-900 dark:bg-sky-950/60 dark:text-sky-300";
	}
	if (value.includes("login")) {
		return "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950/60 dark:text-amber-300";
	}
	return "border-border bg-muted/30 text-muted-foreground";
}

export function formatAuditEntityType(
	t: TFunction,
	entityType: string | null | undefined,
) {
	if (!entityType) {
		return "---";
	}

	return (
		resolveAuditTranslation(
			t,
			`audit_entity_type_${entityType}`,
			"admin",
			entityType,
		) ?? entityType
	);
}
