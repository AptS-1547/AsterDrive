import type { TFunction } from "i18next";
import type { BatchItemError, BatchResult } from "@/types/api";

export type BatchOperation = "move" | "copy" | "delete";

export interface BatchToastContent {
	variant: "success" | "error";
	title: string;
	description?: string;
}

function getEntityLabel(t: TFunction, entityType: string): string {
	switch (entityType) {
		case "file":
			return t("common:file");
		case "folder":
			return t("common:folder");
		default:
			return entityType;
	}
}

function formatBatchErrorDescription(
	t: TFunction,
	errors: BatchItemError[],
): string | undefined {
	if (errors.length === 0) return undefined;

	const visibleErrors = errors.slice(0, 3).map((error) =>
		t("batch_error_detail", {
			entityType: getEntityLabel(t, error.entity_type),
			entityId: error.entity_id,
			error: error.error,
		}),
	);
	const remainingCount = errors.length - visibleErrors.length;

	if (remainingCount > 0) {
		visibleErrors.push(t("batch_error_more", { count: remainingCount }));
	}

	return visibleErrors.join("\n");
}

export function formatBatchToast(
	t: TFunction,
	operation: BatchOperation,
	result: BatchResult,
): BatchToastContent {
	if (result.failed === 0) {
		return {
			variant: "success",
			title: t(`batch_${operation}_success`, { count: result.succeeded }),
		};
	}

	if (result.succeeded === 0) {
		return {
			variant: "error",
			title: t(`batch_${operation}_failed`),
			description: formatBatchErrorDescription(t, result.errors),
		};
	}

	return {
		variant: "success",
		title: t(`batch_${operation}_partial`, {
			succeeded: result.succeeded,
			failed: result.failed,
		}),
		description: formatBatchErrorDescription(t, result.errors),
	};
}
