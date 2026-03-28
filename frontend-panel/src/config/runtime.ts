export const runtimeFlags = {
	appMode: import.meta.env.MODE,
	isDev: import.meta.env.DEV,
	isProd: import.meta.env.PROD,
	showDeveloperErrorDetails: import.meta.env.DEV,
} as const;

export const APP_MODE = runtimeFlags.appMode;
export const IS_DEV = runtimeFlags.isDev;
export const IS_PROD = runtimeFlags.isProd;
export const SHOW_DEVELOPER_ERROR_DETAILS =
	runtimeFlags.showDeveloperErrorDetails;
