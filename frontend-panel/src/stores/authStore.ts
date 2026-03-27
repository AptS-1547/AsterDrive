import axios from "axios";
import { create } from "zustand";
import i18n from "@/i18n";
import { authService } from "@/services/authService";
import { useFileStore } from "@/stores/fileStore";
import { useThemeStore } from "@/stores/themeStore";
import type { ColorPreset, ThemeMode } from "@/stores/themeStore";
import type { UserInfo, UserPreferences } from "@/types/api";
import type { SortBy, SortOrder, ViewMode } from "@/stores/fileStore";

const CACHED_USER_KEY = "aster-cached-user";

function getCachedUser(): UserInfo | null {
	try {
		const raw = localStorage.getItem(CACHED_USER_KEY);
		return raw ? JSON.parse(raw) : null;
	} catch {
		return null;
	}
}

function setCachedUser(user: UserInfo | null) {
	if (user) {
		localStorage.setItem(CACHED_USER_KEY, JSON.stringify(user));
	} else {
		localStorage.removeItem(CACHED_USER_KEY);
	}
}

interface AuthState {
	isAuthenticated: boolean;
	isChecking: boolean;
	isAuthStale: boolean;
	bootOffline: boolean;
	user: UserInfo | null;
	login: (identifier: string, password: string) => Promise<void>;
	logout: () => Promise<void>;
	checkAuth: () => Promise<void>;
	refreshUser: () => Promise<void>;
}

const initialCachedUser = getCachedUser();

function applyServerPreferences(prefs: UserPreferences): void {
	const themeStore = useThemeStore.getState();
	const fileStore = useFileStore.getState();

	themeStore._applyFromServer({
		mode: (prefs.theme_mode as ThemeMode) ?? themeStore.mode,
		colorPreset: (prefs.color_preset as ColorPreset) ?? themeStore.colorPreset,
	});
	fileStore._applyFromServer({
		viewMode: (prefs.view_mode as ViewMode) ?? fileStore.viewMode,
		sortBy: (prefs.sort_by as SortBy) ?? fileStore.sortBy,
		sortOrder: (prefs.sort_order as SortOrder) ?? fileStore.sortOrder,
	});
	if (prefs.language) void i18n.changeLanguage(prefs.language);
}

export const useAuthStore = create<AuthState>((set) => ({
	isAuthenticated: initialCachedUser !== null,
	isChecking: true,
	isAuthStale: initialCachedUser !== null,
	bootOffline: false,
	user: initialCachedUser,

	login: async (identifier, password) => {
		await authService.login(identifier, password);
		const user = await authService.me();
		if (user.preferences) applyServerPreferences(user.preferences);
		setCachedUser(user);
		set({
			isAuthenticated: true,
			isChecking: false,
			isAuthStale: false,
			bootOffline: false,
			user,
		});
	},

	logout: async () => {
		try {
			await authService.logout();
		} catch {
			// logout 失败不阻塞
		}
		setCachedUser(null);
		set({
			isAuthenticated: false,
			isChecking: false,
			isAuthStale: false,
			bootOffline: false,
			user: null,
		});
	},

	checkAuth: async () => {
		set({ isChecking: true, bootOffline: false });
		try {
			const user = await authService.me();
			if (user.preferences) applyServerPreferences(user.preferences);
			setCachedUser(user);
			set({
				isAuthenticated: true,
				isChecking: false,
				isAuthStale: false,
				bootOffline: false,
				user,
			});
		} catch (error) {
			// 网络错误（离线）时用缓存的用户信息保持登录态
			if (!axios.isAxiosError(error) || !error.response) {
				const cached = getCachedUser();
				if (cached) {
					set({
						isAuthenticated: true,
						isChecking: false,
						isAuthStale: true,
						bootOffline: false,
						user: cached,
					});
				} else {
					set({
						isAuthenticated: false,
						isChecking: false,
						isAuthStale: false,
						bootOffline: true,
						user: null,
					});
				}
				return;
			}
			setCachedUser(null);
			set({
				isAuthenticated: false,
				isChecking: false,
				isAuthStale: false,
				bootOffline: false,
				user: null,
			});
		}
	},

	refreshUser: async () => {
		try {
			const user = await authService.me();
			if (user.preferences) applyServerPreferences(user.preferences);
			setCachedUser(user);
			set({
				user,
				isAuthenticated: true,
				isAuthStale: false,
				bootOffline: false,
			});
		} catch {
			// ignore refresh failure; auth interceptors may recover separately
		}
	},
}));
