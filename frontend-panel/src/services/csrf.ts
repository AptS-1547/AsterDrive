export const CSRF_COOKIE_NAME = "aster_csrf";
export const CSRF_HEADER_NAME = "X-CSRF-Token";

export function readCookie(name: string): string | null {
	if (typeof document === "undefined") {
		return null;
	}

	const encodedName = `${encodeURIComponent(name)}=`;
	for (const part of document.cookie.split(";")) {
		const trimmed = part.trim();
		if (!trimmed.startsWith(encodedName)) {
			continue;
		}

		const value = trimmed.slice(encodedName.length);
		return decodeURIComponent(value);
	}

	return null;
}

export function getCsrfToken(): string | null {
	return readCookie(CSRF_COOKIE_NAME);
}
