import { STORAGE_KEYS } from "@/config/app";
import type { FileCategory, OpenWithMode } from "./types";

const OPEN_WITH_PREFERENCE_CATEGORIES: FileCategory[] = [
	"video",
	"markdown",
	"csv",
	"tsv",
	"json",
	"xml",
	"text",
];

type PreferenceMap = Partial<Record<FileCategory, OpenWithMode>>;

function canPersistCategory(category: FileCategory) {
	return OPEN_WITH_PREFERENCE_CATEGORIES.includes(category);
}

function readPreferences(): PreferenceMap {
	if (typeof window === "undefined") return {};
	try {
		const raw = localStorage.getItem(STORAGE_KEYS.previewOpenWith);
		if (!raw) return {};
		return JSON.parse(raw) as PreferenceMap;
	} catch {
		return {};
	}
}

function writePreferences(value: PreferenceMap) {
	if (typeof window === "undefined") return;
	localStorage.setItem(STORAGE_KEYS.previewOpenWith, JSON.stringify(value));
}

export function getStoredOpenWithPreference(category: FileCategory) {
	if (!canPersistCategory(category)) return null;
	const preferences = readPreferences();
	return preferences[category] ?? null;
}

export function setStoredOpenWithPreference(
	category: FileCategory,
	mode: OpenWithMode,
) {
	if (!canPersistCategory(category)) return;
	const preferences = readPreferences();
	preferences[category] = mode;
	writePreferences(preferences);
}
