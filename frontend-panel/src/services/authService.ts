import type {
	ActionMessageResp,
	AuthSessionInfo,
	AuthTokenResp,
	AvatarSource,
	ChangePasswordRequest,
	CheckResp,
	MeField,
	MePartialResponse,
	MeQuery,
	MeResponse,
	PasskeyInfo,
	PasskeyLoginStartRequest,
	PasskeyLoginStartResponse,
	PasskeyRegisterStartRequest,
	PasskeyRegisterStartResponse,
	PasswordResetConfirmRequest,
	PasswordResetRequestRequest,
	PatchPasskeyRequest,
	UpdatePreferencesRequest,
	UpdateProfileRequest,
	UserInfo,
	UserPreferences,
	UserProfileInfo,
} from "@/types/api";
import { type ApiResponse, ErrorCode, isApiSubcode } from "@/types/api-helpers";
import { ApiError, api } from "./http";

export interface AuthSessionState {
	expiresIn: number;
}

interface ListPasskeysOptions {
	force?: boolean;
}

let cachedPasskeys: PasskeyInfo[] | null = null;
let pendingPasskeysRequest: Promise<PasskeyInfo[]> | null = null;
let passkeysCacheSerial = 0;

function clonePasskeys(passkeys: PasskeyInfo[]) {
	return passkeys.map((passkey) => ({ ...passkey }));
}

function primePasskeysCache(passkeys: PasskeyInfo[]) {
	cachedPasskeys = clonePasskeys(passkeys);
}

export function invalidatePasskeysCache() {
	cachedPasskeys = null;
	pendingPasskeysRequest = null;
	passkeysCacheSerial += 1;
}

function upsertCachedPasskey(passkey: PasskeyInfo) {
	pendingPasskeysRequest = null;
	passkeysCacheSerial += 1;
	if (cachedPasskeys === null) {
		return;
	}
	cachedPasskeys = [
		{ ...passkey },
		...cachedPasskeys.filter((item) => item.id !== passkey.id),
	];
}

function replaceCachedPasskey(passkey: PasskeyInfo) {
	pendingPasskeysRequest = null;
	passkeysCacheSerial += 1;
	if (cachedPasskeys === null) {
		return;
	}
	cachedPasskeys = cachedPasskeys.map((item) =>
		item.id === passkey.id ? { ...passkey } : item,
	);
}

function removeCachedPasskey(id: number) {
	pendingPasskeysRequest = null;
	passkeysCacheSerial += 1;
	if (cachedPasskeys === null) {
		return;
	}
	cachedPasskeys = cachedPasskeys.filter((item) => item.id !== id);
}

function listPasskeys(options?: ListPasskeysOptions) {
	const force = options?.force ?? false;
	if (!force && cachedPasskeys !== null) {
		return Promise.resolve(clonePasskeys(cachedPasskeys));
	}
	if (!force && pendingPasskeysRequest !== null) {
		return pendingPasskeysRequest.then(clonePasskeys);
	}

	const requestSerial = ++passkeysCacheSerial;
	const request = api
		.get<PasskeyInfo[]>("/auth/passkeys")
		.then((passkeys) => {
			if (requestSerial === passkeysCacheSerial) {
				primePasskeysCache(passkeys);
			}
			return clonePasskeys(passkeys);
		})
		.finally(() => {
			if (pendingPasskeysRequest === request) {
				pendingPasskeysRequest = null;
			}
		});
	pendingPasskeysRequest = request;
	return request.then(clonePasskeys);
}

function me(): Promise<MeResponse>;
function me(fields: MeField[]): Promise<MePartialResponse>;
function me(fields?: MeField[]) {
	if (!fields || fields.length === 0) {
		return api.get<MeResponse>("/auth/me");
	}
	const params: MeQuery = { fields: fields.join(",") };
	return api.get<MePartialResponse>("/auth/me", {
		params,
	});
}

export const authService = {
	check: () => api.post<CheckResp>("/auth/check"),

	login: async (
		identifier: string,
		password: string,
	): Promise<AuthSessionState> => {
		invalidatePasskeysCache();
		const data = await api.post<AuthTokenResp>("/auth/login", {
			identifier,
			password,
		});
		return {
			expiresIn: Number(data.expires_in) || 900,
		};
	},

	startPasskeyLogin: (payload: PasskeyLoginStartRequest = {}) =>
		api.post<PasskeyLoginStartResponse>("/auth/passkeys/login/start", payload),

	finishPasskeyLogin: async (
		flowId: string,
		credential: unknown,
	): Promise<AuthSessionState> => {
		invalidatePasskeysCache();
		const data = await api.post<AuthTokenResp>("/auth/passkeys/login/finish", {
			flow_id: flowId,
			credential,
		});
		return {
			expiresIn: Number(data.expires_in) || 900,
		};
	},

	register: (username: string, email: string, password: string) => {
		invalidatePasskeysCache();
		return api.post<UserInfo>("/auth/register", { username, email, password });
	},

	resendRegisterActivation: (identifier: string) =>
		api.post<ActionMessageResp>("/auth/register/resend", { identifier }),

	requestPasswordReset: (payload: PasswordResetRequestRequest) =>
		api.post<ActionMessageResp>("/auth/password/reset/request", payload),

	confirmPasswordReset: (payload: PasswordResetConfirmRequest) =>
		api.post<ActionMessageResp>("/auth/password/reset/confirm", payload),

	setup: (username: string, email: string, password: string) => {
		invalidatePasskeysCache();
		return api.post<UserInfo>("/auth/setup", { username, email, password });
	},

	logout: () => {
		invalidatePasskeysCache();
		return api.post<void>("/auth/logout");
	},

	refreshToken: async (): Promise<AuthSessionState> => {
		const data = await api.post<AuthTokenResp>("/auth/refresh");
		return {
			expiresIn: Number(data.expires_in) || 900,
		};
	},

	me,

	updatePreferences: (prefs: UpdatePreferencesRequest) =>
		api.patch<UserPreferences>("/auth/preferences", prefs),

	changePassword: async (
		payload: ChangePasswordRequest,
	): Promise<AuthSessionState> => {
		const data = await api.put<AuthTokenResp>("/auth/password", payload);
		return {
			expiresIn: Number(data.expires_in) || 900,
		};
	},

	listSessions: () => api.get<AuthSessionInfo[]>("/auth/sessions"),

	listPasskeys,

	startPasskeyRegistration: (payload: PasskeyRegisterStartRequest) =>
		api.post<PasskeyRegisterStartResponse>(
			"/auth/passkeys/register/start",
			payload,
		),

	finishPasskeyRegistration: async (
		flowId: string,
		credential: unknown,
		name?: string,
	) => {
		const passkey = await api.post<PasskeyInfo>(
			"/auth/passkeys/register/finish",
			{
				flow_id: flowId,
				credential,
				name,
			},
		);
		upsertCachedPasskey(passkey);
		return passkey;
	},

	renamePasskey: async (id: number, payload: PatchPasskeyRequest) => {
		const passkey = await api.patch<PasskeyInfo>(
			`/auth/passkeys/${id}`,
			payload,
		);
		replaceCachedPasskey(passkey);
		return passkey;
	},

	deletePasskey: async (id: number) => {
		await api.delete<void>(`/auth/passkeys/${id}`);
		removeCachedPasskey(id);
	},

	revokeSession: (id: string) => api.delete<void>(`/auth/sessions/${id}`),

	revokeOtherSessions: async (): Promise<number> => {
		const data = await api.delete<{ removed: number }>("/auth/sessions/others");
		return Number(data.removed) || 0;
	},

	updateProfile: (profile: UpdateProfileRequest) =>
		api.patch<UserProfileInfo>("/auth/profile", profile),

	requestEmailChange: (newEmail: string) =>
		api.post<UserInfo>("/auth/email/change", { new_email: newEmail }),

	resendEmailChange: () =>
		api.post<ActionMessageResp>("/auth/email/change/resend"),

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
			throw new ApiError(resp.code, resp.msg, {
				internalCode: resp.error?.internal_code ?? undefined,
				subcode:
					resp.error?.subcode && isApiSubcode(resp.error.subcode)
						? resp.error.subcode
						: undefined,
				retryable: resp.error?.retryable ?? undefined,
			});
		}
		return resp.data as UserProfileInfo;
	},
};
