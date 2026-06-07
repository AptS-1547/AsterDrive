import { expect } from "@playwright/test";

export const E2E_API_SUCCESS_CODE = "success";

export type E2eApiResponse<T = unknown> = {
	code: unknown;
	data?: T;
	error?: unknown;
	msg?: string;
};

export function expectApiSuccess(payload: E2eApiResponse) {
	expect(payload.code, payload.msg ?? "API response code").toBe(
		E2E_API_SUCCESS_CODE,
	);
}
