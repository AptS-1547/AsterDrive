import type { CheckResp, UpdatePreferencesRequest, UserInfo, UserPreferences } from "@/types/api";
import { api } from "./http";

export const authService = {
	check: (identifier: string) =>
		api.post<CheckResp>("/auth/check", { identifier }),

	login: (identifier: string, password: string) =>
		api.post<null>("/auth/login", { identifier, password }),

	register: (username: string, email: string, password: string) =>
		api.post<UserInfo>("/auth/register", { username, email, password }),

	setup: (username: string, email: string, password: string) =>
		api.post<UserInfo>("/auth/setup", { username, email, password }),

	logout: () => api.post<null>("/auth/logout"),

	me: () => api.get<UserInfo>("/auth/me"),

	updatePreferences: (prefs: UpdatePreferencesRequest) =>
		api.patch<UserPreferences>("/auth/preferences", prefs),
};
