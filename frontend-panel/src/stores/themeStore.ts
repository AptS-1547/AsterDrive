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
type ResolvedTheme = "light" | "dark";

const FALLBACK_THEME_TRANSITION_CLASS = "theme-switching";
const FALLBACK_THEME_TRANSITION_DURATION_MS = 220;

let fallbackThemeTransitionTimer: ReturnType<typeof setTimeout> | null = null;

interface ThemeState {
	mode: ThemeMode;
	colorPreset: ColorPreset;
	resolvedTheme: ResolvedTheme;
	setMode: (mode: ThemeMode) => void;
	setColorPreset: (preset: ColorPreset) => void;
	init: () => void;
	_applyFromServer: (prefs: {
		mode: ThemeMode;
		colorPreset: ColorPreset;
	}) => void;
}

function getStoredValue<T extends string>(key: string, fallback: T): T {
	if (typeof localStorage === "undefined") return fallback;
	return (localStorage.getItem(key) as T) ?? fallback;
}

function prefersDarkMode() {
	if (typeof matchMedia !== "function") return false;
	return matchMedia("(prefers-color-scheme: dark)").matches;
}

function resolveTheme(mode: ThemeMode): ResolvedTheme {
	const isDark = mode === "dark" || (mode === "system" && prefersDarkMode());

	return isDark ? "dark" : "light";
}

function commitTheme(resolvedTheme: ResolvedTheme, preset: ColorPreset) {
	const html = document.documentElement;

	if (resolvedTheme === "dark") {
		html.classList.add("dark");
	} else {
		html.classList.remove("dark");
	}
	html.setAttribute("data-theme", preset);
}

function prefersReducedMotion() {
	if (typeof matchMedia !== "function") return false;
	return matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function clearFallbackThemeTransition() {
	document.documentElement.classList.remove(FALLBACK_THEME_TRANSITION_CLASS);
	if (fallbackThemeTransitionTimer !== null) {
		clearTimeout(fallbackThemeTransitionTimer);
		fallbackThemeTransitionTimer = null;
	}
}

function runThemeTransition(
	updateCallback: () => void,
	options: { animate?: boolean } = {},
) {
	if (
		typeof document === "undefined" ||
		!options.animate ||
		prefersReducedMotion()
	) {
		updateCallback();
		return;
	}

	const html = document.documentElement;
	clearFallbackThemeTransition();
	html.classList.add(FALLBACK_THEME_TRANSITION_CLASS);
	updateCallback();
	fallbackThemeTransitionTimer = setTimeout(() => {
		clearFallbackThemeTransition();
	}, FALLBACK_THEME_TRANSITION_DURATION_MS);
}

function applyTheme(
	mode: ThemeMode,
	preset: ColorPreset,
	options: { animate?: boolean } = {},
): ResolvedTheme {
	const resolvedTheme = resolveTheme(mode);
	runThemeTransition(() => {
		commitTheme(resolvedTheme, preset);
	}, options);
	return resolvedTheme;
}

export type { ColorPreset, ThemeMode };
export { COLOR_PRESETS, THEME_MODES };

const initialMode = getStoredValue(STORAGE_KEYS.themeMode, "system");
const initialColorPreset = getStoredValue(STORAGE_KEYS.colorPreset, "blue");
const initialResolvedTheme = resolveTheme(initialMode);

export const useThemeStore = create<ThemeState>((set, get) => ({
	mode: initialMode,
	colorPreset: initialColorPreset,
	resolvedTheme: initialResolvedTheme,

	setMode: (mode) => {
		localStorage.setItem(STORAGE_KEYS.themeMode, mode);
		const resolved = applyTheme(mode, get().colorPreset, { animate: true });
		set({ mode, resolvedTheme: resolved });
		queuePreferenceSync({ theme_mode: mode });
	},

	setColorPreset: (preset) => {
		localStorage.setItem(STORAGE_KEYS.colorPreset, preset);
		applyTheme(get().mode, preset, { animate: true });
		set({ colorPreset: preset });
		queuePreferenceSync({ color_preset: preset });
	},

	init: () => {
		const { mode, colorPreset } = get();
		const resolved = applyTheme(mode, colorPreset);
		set({ resolvedTheme: resolved });

		if (typeof matchMedia !== "function") return;

		const mq = matchMedia("(prefers-color-scheme: dark)");
		const handler = () => {
			if (get().mode === "system") {
				const r = applyTheme("system", get().colorPreset, { animate: true });
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
