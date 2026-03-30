import type {
	ApiResponse,
	AvatarSource,
	ChangePasswordRequest,
	CheckResp,
	MeResponse,
	UpdatePreferencesRequest,
	UpdateProfileRequest,
	UserInfo,
	UserPreferences,
	UserProfileInfo,
} from "@/types/api";
import { ErrorCode } from "@/types/api";
import { ApiError, api } from "./http";

export const authService = {
	check: (identifier: string) =>
		api.post<CheckResp>("/auth/check", { identifier }),

	login: (identifier: string, password: string) =>
		api.post<void>("/auth/login", { identifier, password }),

	register: (username: string, email: string, password: string) =>
		api.post<UserInfo>("/auth/register", { username, email, password }),

	setup: (username: string, email: string, password: string) =>
		api.post<UserInfo>("/auth/setup", { username, email, password }),

	logout: () => api.post<void>("/auth/logout"),

	me: () => api.get<MeResponse>("/auth/me"),

	updatePreferences: (prefs: UpdatePreferencesRequest) =>
		api.patch<UserPreferences>("/auth/preferences", prefs),

	changePassword: (payload: ChangePasswordRequest) =>
		api.put<void>("/auth/password", payload),

	updateProfile: (profile: UpdateProfileRequest) =>
		api.patch<UserProfileInfo>("/auth/profile", profile),

	setAvatarSource: (source: Extract<AvatarSource, "none" | "gravatar">) =>
		api.put<UserProfileInfo>("/auth/profile/avatar/source", { source }),

	uploadAvatar: async (file: File) => {
		const formData = new FormData();
		formData.set("file", file);
		const { data: resp } = await api.client.post<ApiResponse<UserProfileInfo>>(
			"/auth/profile/avatar/upload",
			formData,
			{
				headers: {
					"Content-Type": "multipart/form-data",
				},
			},
		);
		if (resp.code !== ErrorCode.Success) {
			throw new ApiError(resp.code, resp.msg);
		}
		return resp.data as UserProfileInfo;
	},
};
