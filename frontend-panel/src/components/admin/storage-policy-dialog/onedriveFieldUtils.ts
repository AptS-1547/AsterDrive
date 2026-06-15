import type { OneDriveAccountMode } from "@/types/api";
import type { SharedFieldProps } from "./StoragePolicyFieldTypes";

export const MICROSOFT_GRAPH_PROVIDER = "microsoft_graph";

export const ONE_DRIVE_CUSTOM_TENANT_MODE = "custom";
export const ONE_DRIVE_AUTO_TENANT_MODE = "auto";

export type OneDriveTenantMode =
	| typeof ONE_DRIVE_AUTO_TENANT_MODE
	| "consumers"
	| "organizations"
	| "common"
	| typeof ONE_DRIVE_CUSTOM_TENANT_MODE;

export function getDefaultTenant(mode: OneDriveAccountMode) {
	if (mode === "personal") {
		return "consumers";
	}
	if (mode === "work_or_school") {
		return "common";
	}
	return "organizations";
}

export function getTenantMode(
	form: SharedFieldProps["form"],
): OneDriveTenantMode {
	const tenant = form.onedrive_tenant.trim();
	if (!tenant || tenant === getDefaultTenant(form.onedrive_account_mode)) {
		return ONE_DRIVE_AUTO_TENANT_MODE;
	}
	if (
		tenant === "consumers" ||
		tenant === "organizations" ||
		tenant === "common"
	) {
		return tenant;
	}
	return ONE_DRIVE_CUSTOM_TENANT_MODE;
}

export function formatDateTime(value: string | null | undefined) {
	if (!value) {
		return null;
	}

	try {
		return new Intl.DateTimeFormat(undefined, {
			dateStyle: "medium",
			timeStyle: "short",
		}).format(new Date(value));
	} catch {
		return value;
	}
}
