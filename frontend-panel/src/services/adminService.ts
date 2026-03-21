import type {
	DriverType,
	StoragePolicy,
	SystemConfig,
	UserInfo,
	UserRole,
	UserStatus,
	UserStoragePolicy,
} from "@/types/api";
import { api } from "./http";

// --- Users ---

export const adminUserService = {
	list: () => api.get<UserInfo[]>("/admin/users"),

	get: (id: number) => api.get<UserInfo>(`/admin/users/${id}`),

	update: (
		id: number,
		data: { role?: UserRole; status?: UserStatus; storage_quota?: number },
	) => api.patch<UserInfo>(`/admin/users/${id}`, data),
};

// --- Policies ---

export const adminPolicyService = {
	list: () => api.get<StoragePolicy[]>("/admin/policies"),

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
		is_default?: boolean;
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
			is_default?: boolean;
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
	list: (userId: number) =>
		api.get<UserStoragePolicy[]>(`/admin/users/${userId}/policies`),

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

interface WebdavLock {
	id: number;
	token: string;
	path: string;
	principal: string | null;
	owner_xml: string | null;
	timeout_at: string | null;
	shared: boolean;
	deep: boolean;
	created_at: string;
}

export const adminLockService = {
	list: () => api.get<WebdavLock[]>("/admin/locks"),

	forceUnlock: (id: number) => api.delete<void>(`/admin/locks/${id}`),

	cleanupExpired: () => api.delete<{ removed: number }>("/admin/locks/expired"),
};

// --- System Config ---

export const adminConfigService = {
	list: () => api.get<SystemConfig[]>("/admin/config"),

	get: (key: string) => api.get<SystemConfig>(`/admin/config/${key}`),

	set: (key: string, value: string) =>
		api.put<SystemConfig>(`/admin/config/${key}`, { value }),

	delete: (key: string) => api.delete<void>(`/admin/config/${key}`),
};
