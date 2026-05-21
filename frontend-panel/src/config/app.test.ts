import { afterEach, describe, expect, it, vi } from "vitest";

async function loadAppConfigWithVersion(content?: string) {
	vi.resetModules();
	document.head.innerHTML = "";

	if (content !== undefined) {
		const meta = document.createElement("meta");
		meta.setAttribute("name", "asterdrive-version");
		meta.setAttribute("content", content);
		document.head.append(meta);
	}

	return import("@/config/app");
}

describe("app config", () => {
	afterEach(() => {
		document.head.innerHTML = "";
		vi.resetModules();
	});

	it("reads and trims the embedded app version meta value", async () => {
		const { config } = await loadAppConfigWithVersion("  1.2.3  ");

		expect(config.appName).toBe("AsterDrive");
		expect(config.apiBaseUrl).toBe("/api/v1");
		expect(config.appVersion).toBe("1.2.3");
	});

	it("ignores unresolved template placeholders in the version meta tag", async () => {
		const { config } = await loadAppConfigWithVersion("%ASTERDRIVE_VERSION%");

		expect(config.appVersion).toBe("dev");
	});
});
