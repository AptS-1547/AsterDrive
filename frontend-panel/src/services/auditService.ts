import type { AuditLogPage } from "@/types/api";
import { api } from "@/services/http";

interface AuditLogQuery {
	user_id?: number;
	action?: string;
	entity_type?: string;
	after?: string;
	before?: string;
	limit?: number;
	offset?: number;
}

export const auditService = {
	list: (params: AuditLogQuery = {}) => {
		const query = new URLSearchParams();
		for (const [key, value] of Object.entries(params)) {
			if (value !== undefined && value !== null && value !== "") {
				query.set(key, String(value));
			}
		}
		return api.get<AuditLogPage>(`/admin/audit-logs?${query.toString()}`);
	},
};
