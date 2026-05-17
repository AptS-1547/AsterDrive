import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	AnimateHeight,
	AnimateInlineSwap,
	AnimateMeasuredHeight,
	AnimateSwap,
	AnimateText,
} from "./authAnimations";

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
		vi.unstubAllGlobals();
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

	it("tracks resize observer updates when content is not currently animating", () => {
		let resizeCallback: ResizeObserverCallback | null = null;
		const observe = vi.fn();
		const disconnect = vi.fn();
		class MockResizeObserver {
			constructor(callback: ResizeObserverCallback) {
				resizeCallback = callback;
			}

			observe = observe;
			disconnect = disconnect;
			unobserve = vi.fn();
		}
		const originalResizeObserver = window.ResizeObserver;
		window.ResizeObserver =
			MockResizeObserver as unknown as typeof ResizeObserver;

		const view = render(
			<AnimateMeasuredHeight>
				<div data-height="44">Measured</div>
			</AnimateMeasuredHeight>,
		);
		const container = view.container.firstElementChild as HTMLDivElement;

		expect(observe).toHaveBeenCalledTimes(1);
		act(() => {
			container.style.height = "20px";
			resizeCallback?.([], {} as ResizeObserver);
			container.style.height = "";
			resizeCallback?.([], {} as ResizeObserver);
		});

		view.unmount();
		window.ResizeObserver = originalResizeObserver;

		expect(disconnect).toHaveBeenCalledTimes(1);
	});

	it("shows, hides, and unmounts AnimateHeight content after transitions", () => {
		const view = render(
			<AnimateHeight show={false}>
				<span>recovery-panel</span>
			</AnimateHeight>,
		);

		expect(screen.queryByText("recovery-panel")).not.toBeInTheDocument();

		view.rerender(
			<AnimateHeight show>
				<span>recovery-panel</span>
			</AnimateHeight>,
		);
		expect(screen.getByText("recovery-panel")).toBeInTheDocument();
		const container = view.container.firstElementChild as HTMLDivElement;
		expect(container.style.gridTemplateRows).toBe("0fr");

		act(() => {
			vi.runOnlyPendingTimers();
			vi.runOnlyPendingTimers();
		});
		expect(container.style.gridTemplateRows).toBe("1fr");

		view.rerender(
			<AnimateHeight show={false}>
				<span>recovery-panel</span>
			</AnimateHeight>,
		);
		expect(container.style.gridTemplateRows).toBe("0fr");
		fireEvent.transitionEnd(container);

		expect(screen.queryByText("recovery-panel")).not.toBeInTheDocument();
	});

	it("fades text out before swapping the displayed value", () => {
		const view = render(<AnimateText text="first" className="extra-class" />);

		expect(screen.getByText("first")).toHaveClass("opacity-100");

		view.rerender(<AnimateText text="second" className="extra-class" />);
		expect(screen.getByText("first")).toHaveClass("opacity-0");

		act(() => {
			vi.advanceTimersByTime(150);
		});

		expect(screen.getByText("second")).toHaveClass("opacity-100");
		expect(screen.getByText("second")).toHaveClass("extra-class");
	});

	it("delays block swaps until the outgoing content has faded", () => {
		const view = render(
			<AnimateSwap activeKey="first">
				<span>first-panel</span>
			</AnimateSwap>,
		);
		const animated = view.container.querySelector("[aria-hidden]");
		if (!animated) throw new Error("animated swap element not found");

		expect(screen.getByText("first-panel")).toBeInTheDocument();
		expect(animated).toHaveAttribute("aria-hidden", "false");

		view.rerender(
			<AnimateSwap activeKey="second">
				<span>second-panel</span>
			</AnimateSwap>,
		);
		expect(screen.getByText("first-panel")).toBeInTheDocument();
		expect(animated).toHaveAttribute("aria-hidden", "true");

		act(() => {
			vi.advanceTimersByTime(180);
			vi.runOnlyPendingTimers();
			vi.runOnlyPendingTimers();
		});

		expect(screen.getByText("second-panel")).toBeInTheDocument();
		expect(animated).toHaveAttribute("aria-hidden", "false");
	});

	it("delays inline swaps with the inline hidden state", () => {
		const view = render(
			<AnimateInlineSwap activeKey="first">
				<span>first-label</span>
			</AnimateInlineSwap>,
		);
		const animated = view.container.querySelector("[aria-hidden]");
		if (!animated) throw new Error("animated inline element not found");

		view.rerender(
			<AnimateInlineSwap activeKey="second">
				<span>second-label</span>
			</AnimateInlineSwap>,
		);
		expect(screen.getByText("first-label")).toBeInTheDocument();
		expect(animated).toHaveAttribute("aria-hidden", "true");
		expect(animated).toHaveClass("-translate-y-1");

		act(() => {
			vi.advanceTimersByTime(180);
			vi.runOnlyPendingTimers();
			vi.runOnlyPendingTimers();
		});

		expect(screen.getByText("second-label")).toBeInTheDocument();
		expect(animated).toHaveAttribute("aria-hidden", "false");
	});
});
