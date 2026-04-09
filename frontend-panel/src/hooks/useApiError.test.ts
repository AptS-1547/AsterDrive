import { beforeEach, describe, expect, it, vi } from "vitest";
import { ErrorCode } from "@/types/api-helpers";

const mockState = vi.hoisted(() => {
	class MockApiError extends Error {
		code: number;

		constructor(code: number, message: string) {
			super(message);
			this.code = code;
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
});
