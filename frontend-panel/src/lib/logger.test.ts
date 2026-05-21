import { afterEach, describe, expect, it, vi } from "vitest";

const runtimeFlagsMock = vi.hoisted(() => ({
	isDev: false,
}));

vi.mock("@/config/runtime", () => ({
	runtimeFlags: runtimeFlagsMock,
}));

describe("logger", () => {
	afterEach(() => {
		runtimeFlagsMock.isDev = false;
		vi.restoreAllMocks();
	});

	it("always prefixes warning and error logs", async () => {
		const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
		const error = vi.spyOn(console, "error").mockImplementation(() => {});
		const { logger } = await import("@/lib/logger");

		logger.warn("slow request", { id: 1 });
		logger.error("failed request", { id: 2 });

		expect(warn).toHaveBeenCalledWith("[AsterDrive]", "slow request", {
			id: 1,
		});
		expect(error).toHaveBeenCalledWith("[AsterDrive]", "failed request", {
			id: 2,
		});
	});

	it("only writes debug logs in development mode", async () => {
		const debug = vi.spyOn(console, "debug").mockImplementation(() => {});
		const { logger } = await import("@/lib/logger");

		logger.debug("hidden");
		runtimeFlagsMock.isDev = true;
		logger.debug("visible", 42);

		expect(debug).toHaveBeenCalledTimes(1);
		expect(debug).toHaveBeenCalledWith("[AsterDrive]", "visible", 42);
	});
});
