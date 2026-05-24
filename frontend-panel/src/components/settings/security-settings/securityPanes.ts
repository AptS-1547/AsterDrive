import type { IconName } from "@/components/ui/icon";

export type SecurityPane =
	| "account"
	| "mfa"
	| "passkeys"
	| "external"
	| "sessions";

export const SECURITY_PANES: Array<{
	icon: IconName;
	labelKey: string;
	value: SecurityPane;
}> = [
	{
		icon: "Lock",
		labelKey: "settings:settings_security_tab_account",
		value: "account",
	},
	{
		icon: "Shield",
		labelKey: "settings:settings_security_tab_passkeys",
		value: "passkeys",
	},
	{
		icon: "Key",
		labelKey: "settings:settings_security_tab_mfa",
		value: "mfa",
	},
	{
		icon: "Globe",
		labelKey: "settings:settings_security_tab_external",
		value: "external",
	},
	{
		icon: "Monitor",
		labelKey: "settings:settings_security_tab_sessions",
		value: "sessions",
	},
];
