import { api } from "@/services/http";

interface AuditLogEntry {
	id: number;
	user_id: number;
	action: string;
	entity_type: string | null;
	entity_id: number | null;
	entity_name: string | null;
	details: string | null;
	ip_address: string | null;
	user_agent: string | null;
	created_at: string;
}

interface AuditLogPage {
	items: AuditLogEntry[];
	total: number;
	limit: number;
	offset: number;
}

interface AuditLogQuery {
	user_id?: number;
	action?: string;
	entity_type?: string;
	after?: string;
	before?: string;
	limit?: number;
	offset?: number;
}

export type { AuditLogEntry, AuditLogPage };

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
