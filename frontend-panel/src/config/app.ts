export const config = {
	apiBaseUrl: import.meta.env.VITE_API_BASE_URL || "/api/v1",
	appName: "AsterDrive",
} as const;

export const STORAGE_KEYS = {
	themeMode: "aster-theme-mode",
	colorPreset: "aster-color-preset",
	language: "aster-language",
	viewMode: "aster-view-mode",
	trashViewMode: "aster-trash-view-mode",
	sortBy: "aster-sort-by",
	sortOrder: "aster-sort-order",
	previewOpenWith: "aster-preview-open-with",
} as const;
