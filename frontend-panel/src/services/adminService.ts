import type {
	AdminOverview,
	AdminSharePage,
	DriverType,
	LockPage,
	PolicyGroupUserMigrationResult,
	RemovedCountResponse,
	ResetUserPasswordRequest,
	ShareInfo,
	StoragePolicy,
	StoragePolicyGroup,
	StoragePolicyGroupPage,
	StoragePolicyPage,
	SystemConfig,
	SystemConfigPage,
	UserInfo,
	UserPage,
	UserRole,
	UserStatus,
} from "@/types/api";
import { api } from "./http";

export const adminOverviewService = {
	get: (params?: {
		days?: number;
		timezone?: string;
		event_limit?: number;
	}) => {
		const query = new URLSearchParams();
		if (params?.days != null) query.set("days", String(params.days));
		if (params?.timezone) query.set("timezone", params.timezone);
		if (params?.event_limit != null) {
			query.set("event_limit", String(params.event_limit));
		}
		const suffix = query.toString();
		return api.get<AdminOverview>(
			suffix ? `/admin/overview?${suffix}` : "/admin/overview",
		);
	},
};

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
		data: {
			role?: UserRole;
			status?: UserStatus;
			storage_quota?: number;
			policy_group_id?: number;
		},
	) => api.patch<UserInfo>(`/admin/users/${id}`, data),

	resetPassword: (id: number, data: ResetUserPasswordRequest) =>
		api.put<void>(`/admin/users/${id}/password`, data),

	revokeSessions: (id: number) =>
		api.post<void>(`/admin/users/${id}/sessions/revoke`),

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

// --- Policy Groups ---

export const adminPolicyGroupService = {
	list: (params?: { limit?: number; offset?: number }) => {
		const query = new URLSearchParams();
		if (params?.limit != null) query.set("limit", String(params.limit));
		if (params?.offset != null) query.set("offset", String(params.offset));
		const suffix = query.toString();
		return api.get<StoragePolicyGroupPage>(
			suffix ? `/admin/policy-groups?${suffix}` : "/admin/policy-groups",
		);
	},

	listAll: async (pageSize = 100) => {
		const allGroups: StoragePolicyGroup[] = [];
		let offset = 0;
		let total = 0;

		do {
			const page = await adminPolicyGroupService.list({
				limit: pageSize,
				offset,
			});
			allGroups.push(...page.items);
			total = page.total;
			offset += page.items.length;
			if (page.items.length === 0) {
				break;
			}
		} while (allGroups.length < total);

		return allGroups;
	},

	get: (id: number) =>
		api.get<StoragePolicyGroup>(`/admin/policy-groups/${id}`),

	create: (data: {
		name: string;
		description?: string;
		is_enabled?: boolean;
		is_default?: boolean;
		items: Array<{
			policy_id: number;
			priority: number;
			min_file_size?: number;
			max_file_size?: number;
		}>;
	}) => api.post<StoragePolicyGroup>("/admin/policy-groups", data),

	update: (
		id: number,
		data: {
			name?: string;
			description?: string;
			is_enabled?: boolean;
			is_default?: boolean;
			items?: Array<{
				policy_id: number;
				priority: number;
				min_file_size?: number;
				max_file_size?: number;
			}>;
		},
	) => api.patch<StoragePolicyGroup>(`/admin/policy-groups/${id}`, data),

	delete: (id: number) => api.delete<void>(`/admin/policy-groups/${id}`),

	migrateUsers: (id: number, data: { target_group_id: number }) =>
		api.post<PolicyGroupUserMigrationResult>(
			`/admin/policy-groups/${id}/migrate-users`,
			data,
		),
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
