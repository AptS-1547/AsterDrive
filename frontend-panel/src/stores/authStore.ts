import axios from "axios";
import { create } from "zustand";
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
	bootOffline: boolean;
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
	bootOffline: false,
	user: initialCachedUser,

	login: async (identifier, password) => {
		await authService.login(identifier, password);
		const user = await authService.me();
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
