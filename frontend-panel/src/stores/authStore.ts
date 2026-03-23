import { create } from "zustand";
import { authService } from "@/services/authService";
import type { UserInfo } from "@/types/api";

interface AuthState {
	isAuthenticated: boolean;
	isChecking: boolean;
	user: UserInfo | null;
	login: (identifier: string, password: string) => Promise<void>;
	logout: () => Promise<void>;
	checkAuth: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set) => ({
	isAuthenticated: false,
	isChecking: true,
	user: null,

	login: async (identifier, password) => {
		await authService.login(identifier, password);
		const user = await authService.me();
		set({ isAuthenticated: true, user });
	},

	logout: async () => {
		try {
			await authService.logout();
		} catch {
			// logout 失败不阻塞
		}
		set({ isAuthenticated: false, user: null });
	},

	checkAuth: async () => {
		set({ isChecking: true });
		try {
			const user = await authService.me();
			set({ isAuthenticated: true, isChecking: false, user });
		} catch {
			set({ isAuthenticated: false, isChecking: false, user: null });
		}
	},
}));
