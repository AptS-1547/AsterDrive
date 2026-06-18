import type {
	CreatePolicyRequest,
	DriverType,
	MicrosoftGraphCloud,
	OneDriveAccountMode,
} from "@/types/api";

export interface StorageApplicationConfigForm {
	driver_type: DriverType;
	onedrive_cloud: MicrosoftGraphCloud;
	onedrive_account_mode: OneDriveAccountMode;
	onedrive_tenant: string;
	onedrive_client_id: string;
	onedrive_client_secret: string;
	onedrive_scopes: string;
}

export function buildStorageApplicationConfig(
	form: StorageApplicationConfigForm,
): CreatePolicyRequest["application_config"] | undefined {
	if (form.driver_type !== "one_drive") {
		return undefined;
	}
	return {
		microsoft_graph: buildMicrosoftGraphApplicationConfig(form),
	};
}

function buildMicrosoftGraphApplicationConfig(
	form: StorageApplicationConfigForm,
) {
	const scopes = parseMicrosoftGraphScopes(form.onedrive_scopes);
	return {
		cloud: form.onedrive_cloud,
		tenant: form.onedrive_tenant || undefined,
		client_id: form.onedrive_client_id || undefined,
		client_secret: form.onedrive_client_secret || undefined,
		scopes: scopes.length > 0 ? scopes : undefined,
	};
}

export function parseMicrosoftGraphScopes(value: string) {
	return Array.from(
		new Set(
			value
				.split(/\s+/)
				.map((scope) => scope.trim())
				.filter(Boolean),
		),
	);
}
