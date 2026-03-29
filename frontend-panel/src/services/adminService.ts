import type {
	AdminSharePage,
	DriverType,
	LockPage,
	RemovedCountResponse,
	ShareInfo,
	StoragePolicy,
	StoragePolicyPage,
	SystemConfig,
	SystemConfigPage,
	UserInfo,
	UserPage,
	UserRole,
	UserStatus,
	UserStoragePolicy,
	UserStoragePolicyPage,
} from "@/types/api";
import { api } from "./http";

// --- Users ---

export const adminUserService = {
	list: (params?: {
		limit?: number;
		offset?: number;
		keyword?: string;
		role?: UserRole;
		status?: UserStatus;
	}) => {
		const query = new URLSearchParams();
		if (params?.limit != null) query.set("limit", String(params.limit));
		if (params?.offset != null) query.set("offset", String(params.offset));
		if (params?.keyword) query.set("keyword", params.keyword);
		if (params?.role) query.set("role", params.role);
		if (params?.status) query.set("status", params.status);
		const suffix = query.toString();
		return api.get<UserPage>(
			suffix ? `/admin/users?${suffix}` : "/admin/users",
		);
	},

	get: (id: number) => api.get<UserInfo>(`/admin/users/${id}`),

	create: (data: { username: string; email: string; password: string }) =>
		api.post<UserInfo>("/admin/users", data),

	update: (
		id: number,
		data: { role?: UserRole; status?: UserStatus; storage_quota?: number },
	) => api.patch<UserInfo>(`/admin/users/${id}`, data),

	delete: (id: number) => api.delete<void>(`/admin/users/${id}`),
};

// --- Policies ---

export const adminPolicyService = {
	list: (params?: { limit?: number; offset?: number }) => {
		const query = new URLSearchParams();
		if (params?.limit != null) query.set("limit", String(params.limit));
		if (params?.offset != null) query.set("offset", String(params.offset));
		const suffix = query.toString();
		return api.get<StoragePolicyPage>(
			suffix ? `/admin/policies?${suffix}` : "/admin/policies",
		);
	},

	get: (id: number) => api.get<StoragePolicy>(`/admin/policies/${id}`),

	create: (data: {
		name: string;
		driver_type: DriverType;
		endpoint?: string;
		bucket?: string;
		access_key?: string;
		secret_key?: string;
		base_path?: string;
		max_file_size?: number;
		chunk_size?: number;
		is_default?: boolean;
		options?: string;
	}) => api.post<StoragePolicy>("/admin/policies", data),

	update: (
		id: number,
		data: {
			name?: string;
			endpoint?: string;
			bucket?: string;
			access_key?: string;
			secret_key?: string;
			base_path?: string;
			max_file_size?: number;
			chunk_size?: number;
			is_default?: boolean;
			options?: string;
		},
	) => api.patch<StoragePolicy>(`/admin/policies/${id}`, data),

	delete: (id: number) => api.delete<void>(`/admin/policies/${id}`),

	testConnection: (id: number) => api.post<void>(`/admin/policies/${id}/test`),

	testParams: (data: {
		driver_type: DriverType;
		endpoint?: string;
		bucket?: string;
		access_key?: string;
		secret_key?: string;
		base_path?: string;
	}) => api.post<void>("/admin/policies/test", data),
};

// --- User Storage Policies ---

export const adminUserPolicyService = {
	list: (userId: number, params?: { limit?: number; offset?: number }) => {
		const query = new URLSearchParams();
		if (params?.limit != null) query.set("limit", String(params.limit));
		if (params?.offset != null) query.set("offset", String(params.offset));
		const suffix = query.toString();
		return api.get<UserStoragePolicyPage>(
			suffix
				? `/admin/users/${userId}/policies?${suffix}`
				: `/admin/users/${userId}/policies`,
		);
	},

	assign: (
		userId: number,
		data: { policy_id: number; is_default?: boolean; quota_bytes?: number },
	) => api.post<UserStoragePolicy>(`/admin/users/${userId}/policies`, data),

	update: (
		userId: number,
		id: number,
		data: { is_default?: boolean; quota_bytes?: number },
	) =>
		api.patch<UserStoragePolicy>(`/admin/users/${userId}/policies/${id}`, data),

	remove: (userId: number, id: number) =>
		api.delete<void>(`/admin/users/${userId}/policies/${id}`),
};

// --- WebDAV Locks ---

export type WebdavLock = LockPage["items"][number];
export type AdminShare = ShareInfo;

export const adminShareService = {
	list: (params?: { limit?: number; offset?: number }) => {
		const query = new URLSearchParams();
		if (params?.limit != null) query.set("limit", String(params.limit));
		if (params?.offset != null) query.set("offset", String(params.offset));
		const suffix = query.toString();
		return api.get<AdminSharePage>(
			suffix ? `/admin/shares?${suffix}` : "/admin/shares",
		);
	},

	delete: (id: number) => api.delete<void>(`/admin/shares/${id}`),
};

export const adminLockService = {
	list: (params?: { limit?: number; offset?: number }) => {
		const query = new URLSearchParams();
		if (params?.limit != null) query.set("limit", String(params.limit));
		if (params?.offset != null) query.set("offset", String(params.offset));
		const suffix = query.toString();
		return api.get<LockPage>(
			suffix ? `/admin/locks?${suffix}` : "/admin/locks",
		);
	},

	forceUnlock: (id: number) => api.delete<void>(`/admin/locks/${id}`),

	cleanupExpired: () =>
		api.delete<RemovedCountResponse>("/admin/locks/expired"),
};

// --- System Config ---

export interface ConfigSchemaItem {
	key: string;
	value_type: string;
	default_value: string;
	category: string;
	description: string;
	requires_restart: boolean;
	is_sensitive: boolean;
}

export const adminConfigService = {
	list: (params?: { limit?: number; offset?: number }) => {
		const query = new URLSearchParams();
		if (params?.limit != null) query.set("limit", String(params.limit));
		if (params?.offset != null) query.set("offset", String(params.offset));
		const suffix = query.toString();
		return api.get<SystemConfigPage>(
			suffix ? `/admin/config?${suffix}` : "/admin/config",
		);
	},

	schema: () => api.get<ConfigSchemaItem[]>("/admin/config/schema"),

	get: (key: string) => api.get<SystemConfig>(`/admin/config/${key}`),

	set: (key: string, value: string) =>
		api.put<SystemConfig>(`/admin/config/${key}`, { value }),

	delete: (key: string) => api.delete<void>(`/admin/config/${key}`),
};
