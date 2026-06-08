import { ApiErrorCode, isApiErrorCode } from "@/types/api-helpers";

function readApiCode(value: unknown): string | null {
	if (typeof value !== "object" || value === null) {
		return null;
	}

	const code = "code" in value ? value.code : null;
	return typeof code === "string" && isApiErrorCode(code) ? code : null;
}

function readApiResponseCode(error: unknown): string | null {
	if (typeof error !== "object" || error === null || !("response" in error)) {
		return null;
	}

	const response = error.response;
	if (
		typeof response !== "object" ||
		response === null ||
		!("data" in response)
	) {
		return null;
	}

	return readApiCode(response.data);
}

export function isTokenAuthError(error: unknown): boolean {
	const code = readApiCode(error) ?? readApiResponseCode(error);
	return (
		code === ApiErrorCode.TokenExpired ||
		code === ApiErrorCode.TokenInvalid ||
		code === ApiErrorCode.TokenMissing ||
		code === ApiErrorCode.RefreshTokenReuseDetected
	);
}

function readHttpStatus(error: unknown): number | null {
	if (typeof error !== "object" || error === null || !("response" in error)) {
		return null;
	}

	const response = error.response;
	if (
		typeof response !== "object" ||
		response === null ||
		!("status" in response)
	) {
		return null;
	}

	return typeof response.status === "number" ? response.status : null;
}

export function isSessionAuthFailure(error: unknown): boolean {
	if (isTokenAuthError(error)) {
		return true;
	}

	const status = readHttpStatus(error);
	return status === 401 || status === 403;
}

export function isStaleRefreshTokenError(error: unknown): boolean {
	const code = readApiCode(error) ?? readApiResponseCode(error);
	return code === ApiErrorCode.RefreshTokenStale;
}
