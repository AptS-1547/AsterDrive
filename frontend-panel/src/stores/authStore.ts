import { create } from "zustand";
import axios from "axios";
import { authService } from "@/services/authService";
import type { UserInfo } from "@/types/api";

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
	user: UserInfo | null;
	login: (identifier: string, password: string) => Promise<void>;
	logout: () => Promise<void>;
	checkAuth: () => Promise<void>;
	refreshUser: () => Promise<void>;
}

const initialCachedUser = getCachedUser();

export const useAuthStore = create<AuthState>((set) => ({
	isAuthenticated: initialCachedUser !== null,
	isChecking: true,
	isAuthStale: initialCachedUser !== null,
	user: initialCachedUser,

	login: async (identifier, password) => {
		await authService.login(identifier, password);
		const user = await authService.me();
		setCachedUser(user);
		set({ isAuthenticated: true, isChecking: false, isAuthStale: false, user });
	},

	logout: async () => {
		try {
			await authService.logout();
		} catch {
			// logout 失败不阻塞
		}
		setCachedUser(null);
		set({ isAuthenticated: false, isChecking: false, isAuthStale: false, user: null });
	},

	checkAuth: async () => {
		set({ isChecking: true });
		try {
			const user = await authService.me();
			setCachedUser(user);
			set({ isAuthenticated: true, isChecking: false, isAuthStale: false, user });
		} catch (error) {
			// 网络错误（离线）时用缓存的用户信息保持登录态
			if (!axios.isAxiosError(error) || !error.response) {
				const cached = getCachedUser();
				if (cached) {
					set({
						isAuthenticated: true,
						isChecking: false,
						isAuthStale: true,
						user: cached,
					});
				} else {
					set({ isChecking: false, isAuthStale: false });
				}
				return;
			}
			setCachedUser(null);
			set({ isAuthenticated: false, isChecking: false, isAuthStale: false, user: null });
		}
	},

	refreshUser: async () => {
		try {
			const user = await authService.me();
			setCachedUser(user);
			set({ user, isAuthenticated: true, isAuthStale: false });
		} catch {
			// ignore refresh failure; auth interceptors may recover separately
		}
	},
}));
