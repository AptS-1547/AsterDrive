import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AnimateMeasuredHeight } from "./authAnimations";

const originalGetBoundingClientRect =
	HTMLElement.prototype.getBoundingClientRect;

function createRect(height: number): DOMRect {
	return {
		bottom: height,
		height,
		left: 0,
		right: 320,
		toJSON: () => ({}),
		top: 0,
		width: 320,
		x: 0,
		y: 0,
	} satisfies DOMRect;
}

function measuredHeight(element: Element) {
	if (!(element instanceof HTMLElement)) {
		return 0;
	}

	const explicitHeight = element.dataset.height
		? Number(element.dataset.height)
		: null;
	if (explicitHeight !== null) {
		return explicitHeight;
	}

	const paddingTop = Number.parseFloat(element.style.paddingTop || "0");
	const classPaddingTop = element.classList.contains("pt-4") ? 16 : 0;
	const paddingBottom = Number.parseFloat(element.style.paddingBottom || "0");
	let childrenHeight = 0;
	for (const child of Array.from(element.children)) {
		childrenHeight += measuredHeight(child);
	}
	return paddingTop + classPaddingTop + childrenHeight + paddingBottom;
}

describe("AnimateMeasuredHeight", () => {
	beforeEach(() => {
		vi.useFakeTimers();
		vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
			return window.setTimeout(() => callback(performance.now()), 0);
		});
		vi.spyOn(window, "cancelAnimationFrame").mockImplementation((id) => {
			window.clearTimeout(id);
		});
		HTMLElement.prototype.getBoundingClientRect = function () {
			const measuredElement = this instanceof HTMLElement ? this : null;
			if (
				!measuredElement ||
				(!measuredElement.dataset.height &&
					!measuredElement.classList.contains("pt-4") &&
					!measuredElement.querySelector("[data-height]"))
			) {
				return originalGetBoundingClientRect.call(this);
			}
			return createRect(measuredHeight(measuredElement));
		};
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.useRealTimers();
		HTMLElement.prototype.getBoundingClientRect = originalGetBoundingClientRect;
	});

	it("animates between measured content heights without reserving a fixed blank area", () => {
		const view = render(
			<AnimateMeasuredHeight contentClassName="pt-4">
				<div data-testid="content" data-height="120">
					Password link
				</div>
			</AnimateMeasuredHeight>,
		);
		const container = view.container.firstElementChild as HTMLDivElement;

		view.rerender(
			<AnimateMeasuredHeight contentClassName="pt-4">
				<div data-testid="content" data-height="64">
					Email verification
				</div>
			</AnimateMeasuredHeight>,
		);

		expect(container.style.height).toBe("136px");
		expect(container.style.overflow).toBe("hidden");

		act(() => {
			vi.advanceTimersByTime(0);
		});

		expect(container.style.height).toBe("80px");

		act(() => {
			vi.advanceTimersByTime(220);
		});

		expect(container.style.height).toBe("");
		expect(container.style.overflow).toBe("");
	});
});
