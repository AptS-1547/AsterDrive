import type {
	CreatePolicyGroupRequest,
	StoragePolicy,
	StoragePolicyGroup,
} from "@/types/api";

const BYTES_PER_MB = 1024 * 1024;

let nextRuleKey = 0;

export interface PolicyGroupRuleForm {
	key: string;
	policyId: string;
	priority: string;
	minFileSizeMb: string;
	maxFileSizeMb: string;
	originalMinFileSizeBytes?: number;
	originalMaxFileSizeBytes?: number;
}

export interface PolicyGroupFormData {
	name: string;
	description: string;
	isEnabled: boolean;
	isDefault: boolean;
	items: PolicyGroupRuleForm[];
}

function createRuleKey() {
	nextRuleKey += 1;
	return `policy-group-rule-${nextRuleKey}`;
}

export function bytesToMbInput(bytes: number) {
	if (bytes <= 0) return "";

	const mb = bytes / BYTES_PER_MB;
	return Number.isInteger(mb) ? String(mb) : String(mb);
}

export function mbInputToBytes(value: string, originalBytes?: number) {
	const normalized = value.trim();
	if (!normalized) return 0;

	if (
		originalBytes != null &&
		originalBytes > 0 &&
		normalized === bytesToMbInput(originalBytes)
	) {
		return originalBytes;
	}

	const parsed = Number(normalized);
	if (!Number.isFinite(parsed) || parsed <= 0) return 0;

	return Math.round(parsed * BYTES_PER_MB);
}

export function buildPolicyGroupRuleForm(
	policyId?: number | null,
	priority = 1,
	minFileSize = 0,
	maxFileSize = 0,
): PolicyGroupRuleForm {
	return {
		key: createRuleKey(),
		policyId: policyId != null ? String(policyId) : "",
		priority: String(priority),
		minFileSizeMb: bytesToMbInput(minFileSize),
		maxFileSizeMb: bytesToMbInput(maxFileSize),
		originalMinFileSizeBytes: minFileSize || undefined,
		originalMaxFileSizeBytes: maxFileSize || undefined,
	};
}

export function getDefaultPolicyGroupForm(
	policies: StoragePolicy[],
): PolicyGroupFormData {
	return {
		name: "",
		description: "",
		isEnabled: true,
		isDefault: false,
		items: [buildPolicyGroupRuleForm(policies[0]?.id ?? null)],
	};
}

export function getPolicyGroupForm(
	group: StoragePolicyGroup,
): PolicyGroupFormData {
	return {
		name: group.name,
		description: group.description,
		isEnabled: group.is_enabled,
		isDefault: group.is_default,
		items: group.items.map((item) =>
			buildPolicyGroupRuleForm(
				item.policy_id,
				item.priority,
				item.min_file_size,
				item.max_file_size,
			),
		),
	};
}

export function validatePolicyGroupForm(
	form: PolicyGroupFormData,
	availablePolicyCount: number,
	t: (key: string) => string,
): string | null {
	if (!form.name.trim()) {
		return t("policy_group_name_required");
	}
	if (form.isDefault && !form.isEnabled) {
		return t("policy_group_default_requires_enabled");
	}
	if (availablePolicyCount === 0) {
		return t("policy_group_no_policies_available");
	}
	if (form.items.length === 0) {
		return t("policy_group_rule_required");
	}

	const seenPolicyIds = new Set<string>();
	const seenPriorities = new Set<number>();

	for (const item of form.items) {
		if (!item.policyId) {
			return t("policy_group_rule_policy_required");
		}

		const priority = Number(item.priority);
		if (!Number.isInteger(priority) || priority <= 0) {
			return t("policy_group_rule_priority_invalid");
		}
		if (seenPolicyIds.has(item.policyId)) {
			return t("policy_group_rule_policy_duplicate");
		}
		if (seenPriorities.has(priority)) {
			return t("policy_group_rule_priority_duplicate");
		}

		seenPolicyIds.add(item.policyId);
		seenPriorities.add(priority);

		const min = item.minFileSizeMb.trim() ? Number(item.minFileSizeMb) : 0;
		const max = item.maxFileSizeMb.trim() ? Number(item.maxFileSizeMb) : 0;
		if (!Number.isFinite(min) || !Number.isFinite(max) || min < 0 || max < 0) {
			return t("policy_group_rule_size_invalid");
		}
		if (max > 0 && max <= min) {
			return t("policy_group_rule_range_invalid");
		}
	}

	return null;
}

export function buildPolicyGroupPayload(
	form: PolicyGroupFormData,
): CreatePolicyGroupRequest {
	return {
		name: form.name.trim(),
		description: form.description.trim() || undefined,
		is_enabled: form.isEnabled,
		is_default: form.isDefault,
		items: [...form.items]
			.map((item) => ({
				policy_id: Number(item.policyId),
				priority: Number(item.priority),
				min_file_size: mbInputToBytes(
					item.minFileSizeMb,
					item.originalMinFileSizeBytes,
				),
				max_file_size: mbInputToBytes(
					item.maxFileSizeMb,
					item.originalMaxFileSizeBytes,
				),
			}))
			.sort((a, b) => a.priority - b.priority),
	};
}
