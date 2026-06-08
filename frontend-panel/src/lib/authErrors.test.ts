import { describe, expect, it } from "vitest";
import {
	isSessionAuthFailure,
	isStaleRefreshTokenError,
	isTokenAuthError,
} from "@/lib/authErrors";
import { ApiErrorCode } from "@/types/api-helpers";

describe("isTokenAuthError", () => {
	it("returns false for primitive errors and responses without API data", () => {
		expect(isTokenAuthError(null)).toBe(false);
		expect(isTokenAuthError("token expired")).toBe(false);
		expect(isTokenAuthError({ response: null })).toBe(false);
		expect(isTokenAuthError({ response: { data: "bad" } })).toBe(false);
	});

	it("detects token auth errors from direct and nested API codes", () => {
		expect(isTokenAuthError({ code: ApiErrorCode.TokenExpired })).toBe(true);
		expect(isTokenAuthError({ code: String(ApiErrorCode.TokenExpired) })).toBe(
			true,
		);
		expect(isTokenAuthError({ code: ApiErrorCode.TokenMissing })).toBe(true);
		expect(
			isTokenAuthError({
				response: {
					data: {
						code: ApiErrorCode.TokenInvalid,
					},
				},
			}),
		).toBe(true);
		expect(
			isTokenAuthError({ code: ApiErrorCode.RefreshTokenReuseDetected }),
		).toBe(true);
		expect(isTokenAuthError({ code: ApiErrorCode.CredentialsFailed })).toBe(
			false,
		);
		expect(isTokenAuthError({ code: ApiErrorCode.RefreshTokenStale })).toBe(
			false,
		);
	});

	it("detects stale refresh token errors separately", () => {
		expect(
			isStaleRefreshTokenError({ code: ApiErrorCode.RefreshTokenStale }),
		).toBe(true);
		expect(
			isStaleRefreshTokenError({
				code: String(ApiErrorCode.RefreshTokenStale),
			}),
		).toBe(true);
		expect(
			isStaleRefreshTokenError({
				response: {
					data: {
						code: ApiErrorCode.RefreshTokenStale,
					},
				},
			}),
		).toBe(true);
		expect(isStaleRefreshTokenError({ code: ApiErrorCode.TokenInvalid })).toBe(
			false,
		);
	});

	it("treats only explicit auth failures as session auth failures", () => {
		expect(
			isSessionAuthFailure({ code: ApiErrorCode.RefreshTokenReuseDetected }),
		).toBe(true);
		expect(
			isSessionAuthFailure({
				response: {
					status: 502,
				},
			}),
		).toBe(false);
		expect(
			isSessionAuthFailure({
				response: {
					status: 503,
				},
			}),
		).toBe(false);
		expect(
			isSessionAuthFailure({
				response: {
					status: 504,
				},
			}),
		).toBe(false);
		expect(
			isSessionAuthFailure({
				response: {
					status: 401,
				},
			}),
		).toBe(true);
		expect(
			isSessionAuthFailure({
				response: {
					status: 403,
				},
			}),
		).toBe(true);
		expect(isSessionAuthFailure({ status: 401 })).toBe(true);
		expect(isSessionAuthFailure({ status: 403 })).toBe(true);
		expect(isSessionAuthFailure({ status: 502 })).toBe(false);
		expect(isSessionAuthFailure(null)).toBe(false);
		expect(isSessionAuthFailure("offline")).toBe(false);
		expect(isSessionAuthFailure({ status: "401" })).toBe(false);
		expect(isSessionAuthFailure({ response: null })).toBe(false);
		expect(isSessionAuthFailure({ response: {} })).toBe(false);
		expect(isSessionAuthFailure({ response: { status: "401" } })).toBe(false);
		expect(
			isSessionAuthFailure({
				status: "ignored",
				response: { status: 401 },
			}),
		).toBe(true);
	});
});
