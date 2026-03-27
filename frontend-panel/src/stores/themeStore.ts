import { create } from "zustand";
import { STORAGE_KEYS } from "@/config/app";
import { queuePreferenceSync } from "@/lib/preferenceSync";

const THEME_MODES = {
	light: "light",
	dark: "dark",
	system: "system",
} as const;

const COLOR_PRESETS = {
	blue: "blue",
	green: "green",
	purple: "purple",
	orange: "orange",
} as const;

type ThemeMode = (typeof THEME_MODES)[keyof typeof THEME_MODES];
type ColorPreset = (typeof COLOR_PRESETS)[keyof typeof COLOR_PRESETS];

interface ThemeState {
	mode: ThemeMode;
	colorPreset: ColorPreset;
	resolvedTheme: "light" | "dark";
	setMode: (mode: ThemeMode) => void;
	setColorPreset: (preset: ColorPreset) => void;
	init: () => void;
	_applyFromServer: (prefs: { mode: ThemeMode; colorPreset: ColorPreset }) => void;
}

function getStoredValue<T extends string>(key: string, fallback: T): T {
	if (typeof window === "undefined") return fallback;
	return (localStorage.getItem(key) as T) || fallback;
}

function applyTheme(mode: ThemeMode, preset: ColorPreset): "light" | "dark" {
	const html = document.documentElement;
	const isDark =
		mode === "dark" ||
		(mode === "system" &&
			window.matchMedia("(prefers-color-scheme: dark)").matches);

	if (isDark) {
		html.classList.add("dark");
	} else {
		html.classList.remove("dark");
	}
	html.setAttribute("data-theme", preset);

	return isDark ? "dark" : "light";
}

export type { ColorPreset, ThemeMode };
export { COLOR_PRESETS, THEME_MODES };

export const useThemeStore = create<ThemeState>((set, get) => ({
	mode: getStoredValue(STORAGE_KEYS.themeMode, "system"),
	colorPreset: getStoredValue(STORAGE_KEYS.colorPreset, "blue"),
	resolvedTheme: "light",

	setMode: (mode) => {
		localStorage.setItem(STORAGE_KEYS.themeMode, mode);
		const resolved = applyTheme(mode, get().colorPreset);
		set({ mode, resolvedTheme: resolved });
		queuePreferenceSync({ theme_mode: mode });
	},

	setColorPreset: (preset) => {
		localStorage.setItem(STORAGE_KEYS.colorPreset, preset);
		applyTheme(get().mode, preset);
		set({ colorPreset: preset });
		queuePreferenceSync({ color_preset: preset });
	},

	init: () => {
		const { mode, colorPreset } = get();
		const resolved = applyTheme(mode, colorPreset);
		set({ resolvedTheme: resolved });

		const mq = window.matchMedia("(prefers-color-scheme: dark)");
		const handler = () => {
			if (get().mode === "system") {
				const r = applyTheme("system", get().colorPreset);
				set({ resolvedTheme: r });
			}
		};
		mq.addEventListener("change", handler);
	},

	_applyFromServer: ({ mode, colorPreset }) => {
		localStorage.setItem(STORAGE_KEYS.themeMode, mode);
		localStorage.setItem(STORAGE_KEYS.colorPreset, colorPreset);
		const resolved = applyTheme(mode, colorPreset);
		set({ mode, colorPreset, resolvedTheme: resolved });
	},
}));
