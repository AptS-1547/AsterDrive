import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiSubcode, ErrorCode } from "@/types/api-helpers";

const mockState = vi.hoisted(() => {
	class MockApiError extends Error {
		code: number;
		subcode?: string;

		constructor(code: number, message: string, subcode?: string) {
			super(message);
			this.code = code;
			this.subcode = subcode;
		}
	}

	return {
		ApiError: MockApiError,
		toastError: vi.fn(),
		translate: vi.fn((key: string) => `translated:${key}`),
	};
});

vi.mock("sonner", () => ({
	toast: {
		error: mockState.toastError,
	},
}));

vi.mock("@/i18n", () => ({
	default: {
		t: mockState.translate,
	},
}));

vi.mock("@/services/http", () => ({
	ApiError: mockState.ApiError,
}));

describe("handleApiError", () => {
	beforeEach(() => {
		mockState.toastError.mockReset();
		mockState.translate.mockClear();
	});

	it("maps known ApiError codes to translated messages", async () => {
		const { handleApiError } = await import("@/hooks/useApiError");

		handleApiError(new mockState.ApiError(ErrorCode.Forbidden, "raw message"));
		handleApiError(
			new mockState.ApiError(ErrorCode.PendingActivation, "pending"),
		);

		expect(mockState.translate).toHaveBeenCalledWith("errors:forbidden");
		expect(mockState.translate).toHaveBeenCalledWith(
			"errors:pending_activation",
		);
		expect(mockState.toastError).toHaveBeenCalledWith(
			"translated:errors:forbidden",
		);
		expect(mockState.toastError).toHaveBeenCalledWith(
			"translated:errors:pending_activation",
		);
	});

	it("falls back to subcode translations when the top-level code is generic", async () => {
		const { handleApiError } = await import("@/hooks/useApiError");

		handleApiError(
			new mockState.ApiError(
				ErrorCode.StorageDriverError,
				"Storage Driver Error",
				ApiSubcode.StorageTransient,
			),
		);

		expect(mockState.translate).toHaveBeenCalledWith(
			"errors:storage_transient_failure",
		);
		expect(mockState.toastError).toHaveBeenCalledWith(
			"translated:errors:storage_transient_failure",
		);
	});

	it("maps managed ingress precondition subcodes", async () => {
		const { handleApiError } = await import("@/hooks/useApiError");

		handleApiError(
			new mockState.ApiError(
				ErrorCode.PreconditionFailed,
				"managed ingress required",
				ApiSubcode.ManagedIngressRequired,
			),
		);

		expect(mockState.translate).toHaveBeenCalledWith(
			"errors:managed_ingress_required",
		);
		expect(mockState.toastError).toHaveBeenCalledWith(
			"translated:errors:managed_ingress_required",
		);
	});

	it("prefers subcode translations over generic upload codes", async () => {
		const { handleApiError } = await import("@/hooks/useApiError");

		handleApiError(
			new mockState.ApiError(
				ErrorCode.FileUploadFailed,
				"Upload Failed",
				ApiSubcode.UploadTempFileWriteFailed,
			),
		);

		expect(mockState.translate).toHaveBeenCalledWith(
			"errors:upload_temp_file_write_failed",
		);
		expect(mockState.toastError).toHaveBeenCalledWith(
			"translated:errors:upload_temp_file_write_failed",
		);
	});

	it("uses subcode translations for structured conflict errors", async () => {
		const { handleApiError } = await import("@/hooks/useApiError");

		handleApiError(
			new mockState.ApiError(
				ErrorCode.Conflict,
				"email already exists",
				ApiSubcode.AuthEmailExists,
			),
		);

		expect(mockState.translate).toHaveBeenCalledWith(
			"errors:auth_email_exists",
		);
		expect(mockState.toastError).toHaveBeenCalledWith(
			"translated:errors:auth_email_exists",
		);
	});

	it("ignores unknown dynamic subcodes and falls back to the top-level code", async () => {
		const { handleApiError } = await import("@/hooks/useApiError");

		handleApiError(
			new mockState.ApiError(
				ErrorCode.Forbidden,
				"remote denied this operation",
				"remote.dynamic",
			),
		);

		expect(mockState.translate).toHaveBeenCalledWith("errors:forbidden");
		expect(mockState.toastError).toHaveBeenCalledWith(
			"translated:errors:forbidden",
		);
	});

	it("falls back to the raw message for known subcodes without a local message key", async () => {
		const { handleApiError } = await import("@/hooks/useApiError");

		handleApiError(
			new mockState.ApiError(
				ErrorCode.DatabaseError,
				"remote enrollment is required",
				ApiSubcode.RemoteNodeEnrollmentRequired,
			),
		);

		expect(mockState.toastError).toHaveBeenCalledWith(
			"remote enrollment is required",
		);
	});

	it("falls back to the raw message for unknown errors", async () => {
		const { handleApiError } = await import("@/hooks/useApiError");

		handleApiError(new Error("plain failure"));
		handleApiError("unexpected");

		expect(mockState.toastError).toHaveBeenNthCalledWith(1, "plain failure");
		expect(mockState.toastError).toHaveBeenNthCalledWith(
			2,
			"translated:errors:unexpected_error",
		);
	});

	it("treats blank messages as unexpected errors", async () => {
		const { handleApiError } = await import("@/hooks/useApiError");

		handleApiError(new mockState.ApiError(ErrorCode.Conflict, "   "));
		handleApiError(new Error("\n\t"));

		expect(mockState.toastError).toHaveBeenNthCalledWith(
			1,
			"translated:errors:unexpected_error",
		);
		expect(mockState.toastError).toHaveBeenNthCalledWith(
			2,
			"translated:errors:unexpected_error",
		);
	});
});
