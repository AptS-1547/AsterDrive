import { beforeEach, describe, expect, it } from "vitest";
import { CSRF_COOKIE_NAME, getCsrfToken, readCookie } from "@/services/csrf";

function setTestCookie(cookie: string) {
	// biome-ignore lint/suspicious/noDocumentCookie: jsdom tests need direct cookie mutation.
	document.cookie = cookie;
}

describe("csrf helpers", () => {
	beforeEach(() => {
		setTestCookie("plain=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/");
		setTestCookie(
			`${CSRF_COOKIE_NAME}=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/`,
		);
		setTestCookie(
			"space%20name=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/",
		);
	});

	it("returns the decoded cookie value for an exact name match", () => {
		setTestCookie("plain=value%20with%20spaces; path=/");

		expect(readCookie("plain")).toBe("value with spaces");
	});

	it("matches cookie names after URL encoding", () => {
		setTestCookie("space%20name=encoded-value; path=/");

		expect(readCookie("space name")).toBe("encoded-value");
	});

	it("returns null when the cookie is missing", () => {
		expect(readCookie("missing")).toBeNull();
	});

	it("reads the csrf token from the default cookie name", () => {
		setTestCookie(`${CSRF_COOKIE_NAME}=csrf-token-1; path=/`);

		expect(getCsrfToken()).toBe("csrf-token-1");
	});
});
