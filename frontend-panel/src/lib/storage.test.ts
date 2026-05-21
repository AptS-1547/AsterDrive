import { afterEach, describe, expect, it, vi } from "vitest";
import { readLocalStorage, writeLocalStorage } from "@/lib/storage";

describe("storage helpers", () => {
	afterEach(() => {
		vi.restoreAllMocks();
		localStorage.clear();
	});

	it("reads and writes localStorage values", () => {
		writeLocalStorage("aster:key", "value");

		expect(readLocalStorage("aster:key")).toBe("value");
		expect(readLocalStorage("missing")).toBeNull();
	});

	it("swallows localStorage read and write failures", () => {
		vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
			throw new Error("blocked");
		});
		vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
			throw new Error("full");
		});

		expect(readLocalStorage("aster:key")).toBeNull();
		expect(() => writeLocalStorage("aster:key", "value")).not.toThrow();
	});
});
