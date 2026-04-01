import { describe, expect, it } from "vitest";
import {
	buildPolicyGroupPayload,
	bytesToMbInput,
	getDefaultPolicyGroupForm,
	validatePolicyGroupForm,
} from "@/components/admin/policyGroupDialogShared";
import type { StoragePolicy } from "@/types/api";

const t = (key: string) => key;

describe("policyGroupDialogShared", () => {
	it("creates a default form seeded from the first policy", () => {
		const form = getDefaultPolicyGroupForm([
			{ id: 8, name: "Primary" } as StoragePolicy,
		]);

		expect(form.items).toHaveLength(1);
		expect(form.items[0]?.policyId).toBe("8");
		expect(form.items[0]?.priority).toBe("1");
	});

	it("validates duplicate policies and priorities", () => {
		expect(
			validatePolicyGroupForm(
				{
					name: "Duplicated",
					description: "",
					isEnabled: true,
					isDefault: false,
					items: [
						{
							key: "a",
							policyId: "1",
							priority: "1",
							minFileSizeMb: "",
							maxFileSizeMb: "",
						},
						{
							key: "b",
							policyId: "1",
							priority: "2",
							minFileSizeMb: "",
							maxFileSizeMb: "",
						},
					],
				},
				1,
				t,
			),
		).toBe("policy_group_rule_policy_duplicate");

		expect(
			validatePolicyGroupForm(
				{
					name: "Duplicated priority",
					description: "",
					isEnabled: true,
					isDefault: false,
					items: [
						{
							key: "a",
							policyId: "1",
							priority: "1",
							minFileSizeMb: "",
							maxFileSizeMb: "",
						},
						{
							key: "b",
							policyId: "2",
							priority: "1",
							minFileSizeMb: "",
							maxFileSizeMb: "",
						},
					],
				},
				2,
				t,
			),
		).toBe("policy_group_rule_priority_duplicate");
	});

	it("builds sorted payloads and converts megabytes to bytes", () => {
		expect(
			buildPolicyGroupPayload({
				name: "Tiered",
				description: "Routing rules",
				isEnabled: true,
				isDefault: false,
				items: [
					{
						key: "b",
						policyId: "2",
						priority: "2",
						minFileSizeMb: "10",
						maxFileSizeMb: "",
					},
					{
						key: "a",
						policyId: "1",
						priority: "1",
						minFileSizeMb: "",
						maxFileSizeMb: "10",
					},
				],
			}),
		).toEqual({
			name: "Tiered",
			description: "Routing rules",
			is_enabled: true,
			is_default: false,
			items: [
				{
					policy_id: 1,
					priority: 1,
					min_file_size: 0,
					max_file_size: 10 * 1024 * 1024,
				},
				{
					policy_id: 2,
					priority: 2,
					min_file_size: 10 * 1024 * 1024,
					max_file_size: 0,
				},
			],
		});
	});

	it("preserves exact byte thresholds when editing an existing group", () => {
		const preciseBytes = 12_345;

		expect(
			buildPolicyGroupPayload({
				name: "Tiered",
				description: "",
				isEnabled: true,
				isDefault: false,
				items: [
					{
						key: "a",
						policyId: "1",
						priority: "1",
						minFileSizeMb: bytesToMbInput(preciseBytes),
						maxFileSizeMb: "",
						originalMinFileSizeBytes: preciseBytes,
					},
				],
			}).items[0]?.min_file_size,
		).toBe(preciseBytes);
	});
});
