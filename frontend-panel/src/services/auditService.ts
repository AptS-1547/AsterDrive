import { withQuery } from "@/lib/queryParams";
import { api } from "@/services/http";
import type { AuditLogPage } from "@/types/api";

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
		const { limit, offset, ...filters } = params;

		return api.get<AuditLogPage>(
			withQuery("/admin/audit-logs", {
				limit,
				offset,
				...filters,
			}),
		);
	},
};
