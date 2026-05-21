import { afterEach, describe, expect, it } from "vitest";
import {
	clearDeferredStorageRefresh,
	consumeDeferredStorageRefresh,
	deferStorageRefresh,
	enterStorageRefreshGate,
	isStorageRefreshGateActive,
	leaveStorageRefreshGate,
} from "@/lib/storageRefreshGate";

describe("storageRefreshGate", () => {
	afterEach(() => {
		clearDeferredStorageRefresh();
		while (isStorageRefreshGateActive()) {
			leaveStorageRefreshGate();
		}
	});

	it("tracks nested gate activity without going below zero", () => {
		expect(isStorageRefreshGateActive()).toBe(false);

		enterStorageRefreshGate();
		enterStorageRefreshGate();
		expect(isStorageRefreshGateActive()).toBe(true);

		leaveStorageRefreshGate();
		expect(isStorageRefreshGateActive()).toBe(true);

		leaveStorageRefreshGate();
		expect(isStorageRefreshGateActive()).toBe(false);

		leaveStorageRefreshGate();
		expect(isStorageRefreshGateActive()).toBe(false);
	});

	it("defers, consumes, and clears pending refreshes", () => {
		expect(consumeDeferredStorageRefresh()).toBe(false);

		deferStorageRefresh();
		expect(consumeDeferredStorageRefresh()).toBe(true);
		expect(consumeDeferredStorageRefresh()).toBe(false);

		deferStorageRefresh();
		clearDeferredStorageRefresh();
		expect(consumeDeferredStorageRefresh()).toBe(false);
	});
});
