import { create } from "zustand";
import { STORAGE_KEYS } from "@/config/app";
import { queuePreferenceSync } from "@/lib/preferenceSync";
import { readLocalStorage, writeLocalStorage } from "@/lib/storage";

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

const THEME_MODE_VALUES = Object.values(THEME_MODES);
const COLOR_PRESET_VALUES = Object.values(COLOR_PRESETS);

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
	_applyFromServer: (prefs: { mode?: unknown; colorPreset?: unknown }) => void;
}

function isThemeMode(value: unknown): value is ThemeMode {
	return (
		typeof value === "string" && THEME_MODE_VALUES.includes(value as ThemeMode)
	);
}

function isColorPreset(value: unknown): value is ColorPreset {
	return (
		typeof value === "string" &&
		COLOR_PRESET_VALUES.includes(value as ColorPreset)
	);
}

function normalizeThemeMode(value: unknown, fallback: ThemeMode): ThemeMode {
	return isThemeMode(value) ? value : fallback;
}

function normalizeColorPreset(
	value: unknown,
	fallback: ColorPreset,
): ColorPreset {
	return isColorPreset(value) ? value : fallback;
}

function getStoredThemeMode(key: string, fallback: ThemeMode): ThemeMode {
	return normalizeThemeMode(readLocalStorage(key), fallback);
}

function getStoredColorPreset(key: string, fallback: ColorPreset): ColorPreset {
	return normalizeColorPreset(readLocalStorage(key), fallback);
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

const initialMode = getStoredThemeMode(STORAGE_KEYS.themeMode, "system");
const initialColorPreset = getStoredColorPreset(
	STORAGE_KEYS.colorPreset,
	"blue",
);
const initialResolvedTheme = resolveTheme(initialMode);

export const useThemeStore = create<ThemeState>((set, get) => ({
	mode: initialMode,
	colorPreset: initialColorPreset,
	resolvedTheme: initialResolvedTheme,

	setMode: (mode) => {
		const nextMode = normalizeThemeMode(mode, get().mode);
		if (nextMode !== mode) return;
		writeLocalStorage(STORAGE_KEYS.themeMode, nextMode);
		const resolved = applyTheme(nextMode, get().colorPreset, { animate: true });
		set({ mode: nextMode, resolvedTheme: resolved });
		queuePreferenceSync({ theme_mode: nextMode });
	},

	setColorPreset: (preset) => {
		const nextPreset = normalizeColorPreset(preset, get().colorPreset);
		if (nextPreset !== preset) return;
		writeLocalStorage(STORAGE_KEYS.colorPreset, nextPreset);
		applyTheme(get().mode, nextPreset, { animate: true });
		set({ colorPreset: nextPreset });
		queuePreferenceSync({ color_preset: nextPreset });
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
		const nextMode = normalizeThemeMode(mode, get().mode);
		const nextColorPreset = normalizeColorPreset(
			colorPreset,
			get().colorPreset,
		);
		writeLocalStorage(STORAGE_KEYS.themeMode, nextMode);
		writeLocalStorage(STORAGE_KEYS.colorPreset, nextColorPreset);
		const resolved = applyTheme(nextMode, nextColorPreset);
		set({
			mode: nextMode,
			colorPreset: nextColorPreset,
			resolvedTheme: resolved,
		});
	},
}));
