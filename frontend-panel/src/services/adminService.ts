import { withQuery } from "@/lib/queryParams";
import type {
	AdminOverview,
	AdminSharePage,
	CreatePolicyGroupRequest,
	CreatePolicyRequest,
	CreateUserReq,
	DriverType,
	LockPage,
	MigratePolicyGroupUsersRequest,
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
	UpdatePolicyGroupRequest,
	UpdatePolicyRequest,
	UpdateUserRequest,
	UserInfo,
	UserPage,
	UserRole,
	UserStatus,
} from "@/types/api";
import { api } from "./http";

// The admin PATCH endpoint rejects `policy_group_id: null`, and current callers
// only support assigning a group or leaving it unchanged. Strip accidental nulls
// here so broader callers cannot request an unsupported clear operation.
function sanitizeUpdateUserRequest(data: UpdateUserRequest): UpdateUserRequest {
	const rawData = data as UpdateUserRequest & {
		policy_group_id?: number | null;
	};
	if (rawData.policy_group_id != null) {
		return data;
	}

	const { policy_group_id: _policyGroupId, ...payload } = rawData;
	return payload;
}

export const adminOverviewService = {
	get: (params?: { days?: number; timezone?: string; event_limit?: number }) =>
		api.get<AdminOverview>(
			withQuery("/admin/overview", {
				days: params?.days,
				timezone: params?.timezone,
				event_limit: params?.event_limit,
			}),
		),
};

// --- Users ---

export const adminUserService = {
	list: (params?: {
		limit?: number;
		offset?: number;
		keyword?: string;
		role?: UserRole;
		status?: UserStatus;
	}) =>
		api.get<UserPage>(
			withQuery("/admin/users", {
				limit: params?.limit,
				offset: params?.offset,
				keyword: params?.keyword,
				role: params?.role,
				status: params?.status,
			}),
		),

	get: (id: number) => api.get<UserInfo>(`/admin/users/${id}`),

	create: (data: CreateUserReq) => api.post<UserInfo>("/admin/users", data),

	update: (id: number, data: UpdateUserRequest) =>
		api.patch<UserInfo>(`/admin/users/${id}`, sanitizeUpdateUserRequest(data)),

	resetPassword: (id: number, data: ResetUserPasswordRequest) =>
		api.put<void>(`/admin/users/${id}/password`, data),

	revokeSessions: (id: number) =>
		api.post<void>(`/admin/users/${id}/sessions/revoke`),

	delete: (id: number) => api.delete<void>(`/admin/users/${id}`),
};

// --- Policies ---

export const adminPolicyService = {
	list: (params?: { limit?: number; offset?: number }) =>
		api.get<StoragePolicyPage>(
			withQuery("/admin/policies", {
				limit: params?.limit,
				offset: params?.offset,
			}),
		),

	get: (id: number) => api.get<StoragePolicy>(`/admin/policies/${id}`),

	create: (data: CreatePolicyRequest) =>
		api.post<StoragePolicy>("/admin/policies", data),

	update: (id: number, data: UpdatePolicyRequest) =>
		api.patch<StoragePolicy>(`/admin/policies/${id}`, data),

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
	list: (params?: { limit?: number; offset?: number }) =>
		api.get<StoragePolicyGroupPage>(
			withQuery("/admin/policy-groups", {
				limit: params?.limit,
				offset: params?.offset,
			}),
		),

	listAll: async (pageSize = 100) => {
		if (!Number.isInteger(pageSize) || pageSize <= 0) {
			throw new Error("pageSize must be a positive integer");
		}

		const allGroups: StoragePolicyGroup[] = [];
		let offset = 0;
		let total = 0;
		let pageCount = 0;
		let maxPages = Number.POSITIVE_INFINITY;

		do {
			pageCount += 1;
			if (pageCount > maxPages) {
				throw new Error("pagination exceeded max iterations");
			}

			const previousOffset = offset;
			const previousCount = allGroups.length;
			const page = await adminPolicyGroupService.list({
				limit: pageSize,
				offset,
			});
			allGroups.push(...page.items);
			total = page.total;
			maxPages = Math.max(1, Math.ceil(total / pageSize)) + 2;
			offset += page.items.length;
			if (page.items.length === 0) {
				if (allGroups.length < total) {
					throw new Error("incomplete pages from adminPolicyGroupService.list");
				}
				break;
			}
			if (offset <= previousOffset || allGroups.length <= previousCount) {
				throw new Error("pagination did not make progress");
			}
		} while (allGroups.length < total);

		return allGroups;
	},

	get: (id: number) =>
		api.get<StoragePolicyGroup>(`/admin/policy-groups/${id}`),

	create: (data: CreatePolicyGroupRequest) =>
		api.post<StoragePolicyGroup>("/admin/policy-groups", data),

	update: (id: number, data: UpdatePolicyGroupRequest) =>
		api.patch<StoragePolicyGroup>(`/admin/policy-groups/${id}`, data),

	delete: (id: number) => api.delete<void>(`/admin/policy-groups/${id}`),

	migrateUsers: (id: number, data: MigratePolicyGroupUsersRequest) =>
		api.post<PolicyGroupUserMigrationResult>(
			`/admin/policy-groups/${id}/migrate-users`,
			data,
		),
};

// --- WebDAV Locks ---

export type WebdavLock = LockPage["items"][number];
export type AdminShare = ShareInfo;

export const adminShareService = {
	list: (params?: { limit?: number; offset?: number }) =>
		api.get<AdminSharePage>(
			withQuery("/admin/shares", {
				limit: params?.limit,
				offset: params?.offset,
			}),
		),

	delete: (id: number) => api.delete<void>(`/admin/shares/${id}`),
};

export const adminLockService = {
	list: (params?: { limit?: number; offset?: number }) =>
		api.get<LockPage>(
			withQuery("/admin/locks", {
				limit: params?.limit,
				offset: params?.offset,
			}),
		),

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
	list: (params?: { limit?: number; offset?: number }) =>
		api.get<SystemConfigPage>(
			withQuery("/admin/config", {
				limit: params?.limit,
				offset: params?.offset,
			}),
		),

	schema: () => api.get<ConfigSchemaItem[]>("/admin/config/schema"),

	get: (key: string) => api.get<SystemConfig>(`/admin/config/${key}`),

	set: (key: string, value: string) =>
		api.put<SystemConfig>(`/admin/config/${key}`, { value }),

	delete: (key: string) => api.delete<void>(`/admin/config/${key}`),
};
