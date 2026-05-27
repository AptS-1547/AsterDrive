import { describe, expect, it } from "vitest";
import type { TaskInfo } from "@/types/api";
import {
	formatTaskKind,
	formatTaskStepTitle,
	parseStoragePolicyMigrationResult,
} from "./taskPresentation";

function t(key: string) {
	const translations: Record<string, string> = {
		"tasks:kind_storage_policy_migration": "Storage policy migration",
		"tasks:step_storage_policy_migration_prepare_sources":
			"Prepare source policy",
		"tasks:step_storage_policy_migration_scan_blobs": "Scan source blobs",
		"tasks:step_storage_policy_migration_finish": "Finish migration",
		"tasks:step_thumbnail_generate_waiting": "Waiting",
		"tasks:step_thumbnail_generate_inspect_source": "Inspect source file",
		"tasks:step_thumbnail_generate_render_thumbnail": "Render thumbnail",
		"tasks:step_thumbnail_generate_persist_thumbnail": "Save thumbnail",
	};
	return translations[key] ?? key;
}

function createTask(overrides: Partial<TaskInfo> = {}): TaskInfo {
	return {
		attempt_count: 0,
		can_retry: false,
		created_at: "2026-04-17T00:00:00Z",
		creator: null,
		display_name: "Migrate blobs",
		expires_at: "2026-04-18T00:00:00Z",
		finished_at: null,
		id: 31,
		kind: "storage_policy_migration",
		last_error: null,
		max_attempts: 1,
		payload: {
			delete_source_after_success: false,
			kind: "storage_policy_migration",
			plan_hash: "plan-a",
			source_policy_id: 1,
			source_policy_updated_at: "2026-04-17T00:00:00Z",
			target_policy_id: 2,
			target_policy_updated_at: "2026-04-17T00:00:00Z",
		},
		progress_current: 0,
		progress_percent: 0,
		progress_total: 0,
		result: null,
		share_id: null,
		started_at: null,
		status: "pending",
		status_text: null,
		steps: [],
		team_id: null,
		updated_at: "2026-04-17T00:00:00Z",
		...overrides,
	};
}

describe("taskPresentation storage policy migration", () => {
	it("formats the storage policy migration kind", () => {
		expect(formatTaskKind(t, "storage_policy_migration")).toBe(
			"Storage policy migration",
		);
	});

	it("translates known storage migration steps and falls back to backend titles", () => {
		expect(
			formatTaskStepTitle(t, "storage_policy_migration", {
				key: "prepare_sources",
				title: "Backend prepare title",
				status: "succeeded",
				progress_current: 1,
				progress_total: 1,
			}),
		).toBe("Prepare source policy");
		expect(
			formatTaskStepTitle(t, "storage_policy_migration", {
				key: "scan_blobs",
				title: "Scan blobs",
				status: "active",
				progress_current: 3,
				progress_total: 10,
			}),
		).toBe("Scan source blobs");
		expect(
			formatTaskStepTitle(t, "storage_policy_migration", {
				key: "finish",
				title: "Backend finish title",
				status: "succeeded",
				progress_current: 1,
				progress_total: 1,
			}),
		).toBe("Finish migration");
		expect(
			formatTaskStepTitle(t, "storage_policy_migration", {
				key: "custom_backend_step",
				title: "Backend custom step",
				status: "pending",
				progress_current: 0,
				progress_total: 0,
			}),
		).toBe("Backend custom step");
	});

	it("translates thumbnail generation steps", () => {
		expect(
			formatTaskStepTitle(t, "thumbnail_generate", {
				key: "waiting",
				title: "step_thumbnail_generate_waiting",
				status: "succeeded",
				progress_current: 1,
				progress_total: 1,
			}),
		).toBe("Waiting");
		expect(
			formatTaskStepTitle(t, "thumbnail_generate", {
				key: "inspect_source",
				title: "step_thumbnail_generate_inspect_source",
				status: "succeeded",
				progress_current: 1,
				progress_total: 1,
			}),
		).toBe("Inspect source file");
		expect(
			formatTaskStepTitle(t, "thumbnail_generate", {
				key: "render_thumbnail",
				title: "step_thumbnail_generate_render_thumbnail",
				status: "succeeded",
				progress_current: 1,
				progress_total: 1,
			}),
		).toBe("Render thumbnail");
		expect(
			formatTaskStepTitle(t, "thumbnail_generate", {
				key: "persist_thumbnail",
				title: "step_thumbnail_generate_persist_thumbnail",
				status: "succeeded",
				progress_current: 1,
				progress_total: 1,
			}),
		).toBe("Save thumbnail");
	});

	it("parses storage migration results and ignores other result shapes", () => {
		const migrationResult = {
			failed_blobs: 0,
			kind: "storage_policy_migration",
			merged_blobs: 1,
			migrated_blobs: 12,
			migrated_bytes: 4096,
			scanned_blobs: 13,
			skipped_blobs: 0,
			source_policy_id: 1,
			target_policy_id: 2,
		} as const;

		expect(
			parseStoragePolicyMigrationResult(
				createTask({
					result: migrationResult,
					status: "succeeded",
				}),
			),
		).toEqual(migrationResult);
		expect(parseStoragePolicyMigrationResult(createTask())).toBeNull();
		expect(
			parseStoragePolicyMigrationResult(
				createTask({
					kind: "archive_extract",
					result: {
						kind: "archive_extract",
						target_folder_id: 2,
						target_path: "/archive",
					},
				}),
			),
		).toBeNull();
	});
});
