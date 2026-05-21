import { afterEach, describe, expect, it, vi } from "vitest";
import { runWhenIdle } from "@/lib/idleTask";

describe("runWhenIdle", () => {
	afterEach(() => {
		vi.useRealTimers();
		vi.restoreAllMocks();
		delete (window as unknown as { requestIdleCallback?: unknown })
			.requestIdleCallback;
		delete (window as unknown as { cancelIdleCallback?: unknown })
			.cancelIdleCallback;
	});

	it("uses requestIdleCallback when available and cancels it", () => {
		const task = vi.fn();
		const cancelIdleCallback = vi.fn();
		const requestIdleCallback = vi.fn().mockReturnValue(42);
		Object.defineProperty(window, "requestIdleCallback", {
			configurable: true,
			value: requestIdleCallback,
		});
		Object.defineProperty(window, "cancelIdleCallback", {
			configurable: true,
			value: cancelIdleCallback,
		});

		const cancel = runWhenIdle(task, { timeoutMs: 500 });
		cancel();

		expect(requestIdleCallback).toHaveBeenCalledWith(task, { timeout: 500 });
		expect(cancelIdleCallback).toHaveBeenCalledWith(42);
		expect(task).not.toHaveBeenCalled();
	});

	it("falls back to setTimeout and clears the timeout", () => {
		vi.useFakeTimers();
		const task = vi.fn();

		const cancel = runWhenIdle(task, { fallbackDelayMs: 25 });
		vi.advanceTimersByTime(24);
		expect(task).not.toHaveBeenCalled();
		vi.advanceTimersByTime(1);
		expect(task).toHaveBeenCalledTimes(1);

		const secondTask = vi.fn();
		const secondCancel = runWhenIdle(secondTask, { fallbackDelayMs: 25 });
		secondCancel();
		vi.advanceTimersByTime(25);
		expect(secondTask).not.toHaveBeenCalled();
		cancel();
	});
});
