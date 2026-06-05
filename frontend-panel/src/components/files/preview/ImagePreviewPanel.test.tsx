import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ImagePreviewPanel } from "@/components/files/preview/ImagePreviewPanel";
import type { ImagePreviewSource, ShowOriginalState } from "./BlobImagePreview";

const mockState = vi.hoisted(() => ({
	blobProps: null as null | {
		imageStyle?: React.CSSProperties;
		onShowOriginalStateChange?: (state: ShowOriginalState) => void;
		onSourceChange?: (source: ImagePreviewSource) => void;
		showOriginalRequestId?: number;
	},
}));

vi.mock("@/components/files/preview/BlobImagePreview", () => ({
	BlobImagePreview: ({
		imageRef,
		imageStyle,
		onShowOriginalStateChange,
		onSourceChange,
		showOriginalRequestId,
		viewportRef,
	}: {
		imageRef?: React.Ref<HTMLImageElement>;
		imageStyle?: React.CSSProperties;
		onShowOriginalStateChange?: (state: ShowOriginalState) => void;
		onSourceChange?: (source: ImagePreviewSource) => void;
		showOriginalRequestId?: number;
		viewportRef?: React.Ref<HTMLDivElement>;
	}) => {
		mockState.blobProps = {
			imageStyle,
			onShowOriginalStateChange,
			onSourceChange,
			showOriginalRequestId,
		};
		return (
			<div data-testid="panel-preview-viewport" ref={viewportRef}>
				<img
					alt="panel-preview"
					data-testid="panel-preview-image"
					ref={imageRef}
					src="blob:preview"
					style={imageStyle}
				/>
			</div>
		);
	},
}));

vi.mock("@/lib/format", () => ({
	formatBytes: (value: number) => `${value} bytes`,
}));

const file = {
	id: 7,
	mime_type: "image/png",
	name: "photo.png",
	size: 2048,
};

function renderPanel(
	overrides: Partial<React.ComponentProps<typeof ImagePreviewPanel>> = {},
) {
	const props: React.ComponentProps<typeof ImagePreviewPanel> = {
		file,
		allOptionsCount: 1,
		downloadPath: "/files/7/download",
		imagePreviewPath: "/files/7/image-preview",
		isExpanded: true,
		onChooseOpenMethod: vi.fn(),
		onClose: vi.fn(),
		onToggleExpand: vi.fn(),
		chooseOpenMethodLabel: "Choose open method",
		enterFullscreenLabel: "Fill window",
		exitFullscreenLabel: "Restore window",
		closeLabel: "Close",
		fitToWindowLabel: "Fit to window",
		previewSourceLabel: "Preview",
		originalSourceLabel: "Original",
		rotateRightLabel: "Rotate right",
		zoomInLabel: "Zoom in",
		zoomOutLabel: "Zoom out",
		...overrides,
	};

	render(<ImagePreviewPanel {...props} />);
	return props;
}

describe("ImagePreviewPanel", () => {
	beforeEach(() => {
		mockState.blobProps = null;
		Object.defineProperty(HTMLElement.prototype, "setPointerCapture", {
			configurable: true,
			value: vi.fn(),
		});
		Object.defineProperty(HTMLElement.prototype, "hasPointerCapture", {
			configurable: true,
			value: vi.fn(() => true),
		});
		Object.defineProperty(HTMLElement.prototype, "releasePointerCapture", {
			configurable: true,
			value: vi.fn(),
		});
		vi.useRealTimers();
	});

	it("renders media viewer chrome and forwards preview paths", () => {
		renderPanel();

		expect(screen.getByText("photo.png")).toBeInTheDocument();
		expect(screen.getByText("2048 bytes · image/png")).toBeInTheDocument();
		expect(screen.getByText("Original")).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Restore window" }),
		).toBeInTheDocument();
		expect(mockState.blobProps?.showOriginalRequestId).toBeUndefined();
	});

	it("keeps the top and bottom chrome position stable during close fade", () => {
		renderPanel();

		const topChrome = screen.getByText("photo.png").closest(".absolute");
		const bottomChrome = screen
			.getByRole("button", { name: "Fit to window" })
			.closest(".absolute");

		expect(topChrome?.className).toContain("transition-opacity");
		expect(topChrome?.className).not.toContain("translate-y");
		expect(bottomChrome?.className).toContain("transition-opacity");
		expect(bottomChrome?.className).not.toContain("translate-y");
	});

	it("shows open-method control only when multiple methods exist", () => {
		const props = renderPanel({ allOptionsCount: 2 });

		fireEvent.click(screen.getByRole("button", { name: "Choose open method" }));

		expect(props.onChooseOpenMethod).toHaveBeenCalledTimes(1);
	});

	it("hides open-method control when there is only one method", () => {
		renderPanel({ allOptionsCount: 1 });

		expect(
			screen.queryByRole("button", { name: "Choose open method" }),
		).not.toBeInTheDocument();
	});

	it("toggles fullscreen and closes through toolbar buttons", () => {
		const props = renderPanel();

		fireEvent.click(screen.getByRole("button", { name: "Restore window" }));
		fireEvent.click(screen.getByRole("button", { name: "Close" }));

		expect(props.onToggleExpand).toHaveBeenCalledTimes(1);
		expect(props.onClose).toHaveBeenCalledTimes(1);
	});

	it("updates the source badge when the image source changes", () => {
		renderPanel();

		act(() => {
			mockState.blobProps?.onSourceChange?.("backend_preview");
		});

		expect(screen.getByText("Preview")).toBeInTheDocument();
	});

	it("zooms in, zooms out, and resets to fit", () => {
		renderPanel();

		fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
		expect(
			screen.getByRole("button", { name: "Fit to window" }),
		).toHaveTextContent("125%");
		expect(mockState.blobProps?.imageStyle?.transform).toContain("scale(1.25)");

		fireEvent.click(screen.getByRole("button", { name: "Zoom out" }));
		expect(
			screen.getByRole("button", { name: "Fit to window" }),
		).toHaveTextContent("100%");

		fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
		fireEvent.click(screen.getByRole("button", { name: "Fit to window" }));
		expect(
			screen.getByRole("button", { name: "Fit to window" }),
		).toHaveTextContent("100%");
	});

	it("clamps zoom controls at the min and max edges", () => {
		renderPanel();

		for (let index = 0; index < 12; index += 1) {
			fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
		}
		expect(
			screen.getByRole("button", { name: "Fit to window" }),
		).toHaveTextContent("300%");
		expect(screen.getByRole("button", { name: "Zoom in" })).toBeDisabled();

		for (let index = 0; index < 20; index += 1) {
			fireEvent.click(screen.getByRole("button", { name: "Zoom out" }));
		}
		expect(
			screen.getByRole("button", { name: "Fit to window" }),
		).toHaveTextContent("50%");
		expect(screen.getByRole("button", { name: "Zoom out" })).toBeDisabled();
	});

	it("rotates right and resets rotation when fitting to window", () => {
		renderPanel();

		fireEvent.click(screen.getByRole("button", { name: "Rotate right" }));
		expect(mockState.blobProps?.imageStyle?.transform).toContain(
			"rotate(90deg)",
		);

		fireEvent.click(screen.getByRole("button", { name: "Fit to window" }));
		expect(mockState.blobProps?.imageStyle?.transform).toContain(
			"rotate(0deg)",
		);
	});

	it("zooms with ctrl wheel and ignores plain scroll", () => {
		renderPanel();
		const surface = getGestureSurface();

		fireEvent.wheel(surface, {
			clientX: 200,
			clientY: 150,
			ctrlKey: false,
			deltaY: -100,
		});
		expect(
			screen.getByRole("button", { name: "Fit to window" }),
		).toHaveTextContent("100%");

		fireEvent.wheel(surface, {
			clientX: 200,
			clientY: 150,
			ctrlKey: true,
			deltaY: -100,
		});

		expect(
			screen.getByRole("button", { name: "Fit to window" }),
		).toHaveTextContent("125%");
		expect(mockState.blobProps?.imageStyle?.transform).toContain("scale(1.25)");
	});

	it("does not drag the image while it is fitted", () => {
		renderPanel();
		mockImageGeometry();
		const surface = getGestureSurface();

		fireEvent.pointerDown(surface, {
			clientX: 100,
			clientY: 100,
			pointerId: 1,
		});
		fireEvent.pointerMove(surface, {
			clientX: 220,
			clientY: 180,
			pointerId: 1,
		});

		expect(mockState.blobProps?.imageStyle?.transform).toContain(
			"translate3d(0px, 0px, 0)",
		);
	});

	it("clamps drag movement to the visible zoomed image bounds", () => {
		renderPanel();
		mockImageGeometry();
		for (let index = 0; index < 8; index += 1) {
			fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
		}
		expect(
			screen.getByRole("button", { name: "Fit to window" }),
		).toHaveTextContent("300%");

		const surface = getGestureSurface();
		fireEvent.pointerDown(surface, {
			clientX: 200,
			clientY: 150,
			pointerId: 1,
		});
		fireEvent.pointerMove(surface, {
			clientX: 1200,
			clientY: 900,
			pointerId: 1,
		});

		expect(mockState.blobProps?.imageStyle?.transform).toContain(
			"translate3d(250px, 150px, 0)",
		);

		fireEvent.pointerMove(surface, {
			clientX: -1200,
			clientY: -900,
			pointerId: 1,
		});

		expect(mockState.blobProps?.imageStyle?.transform).toContain(
			"translate3d(-250px, -150px, 0)",
		);
	});

	it("pinch-zooms with two pointers and clamps the resulting scale", () => {
		renderPanel();
		mockImageGeometry();
		const surface = getGestureSurface();

		fireEvent.pointerDown(surface, {
			clientX: 180,
			clientY: 150,
			pointerId: 1,
		});
		fireEvent.pointerDown(surface, {
			clientX: 220,
			clientY: 150,
			pointerId: 2,
		});
		fireEvent.pointerMove(surface, {
			clientX: -200,
			clientY: 150,
			pointerId: 1,
		});
		fireEvent.pointerMove(surface, {
			clientX: 600,
			clientY: 150,
			pointerId: 2,
		});

		expect(
			screen.getByRole("button", { name: "Fit to window" }),
		).toHaveTextContent("300%");
		expect(mockState.blobProps?.imageStyle?.transform).toContain("scale(3)");
	});

	it("requests the original and renders loading and success states with collapse animation classes", () => {
		vi.useFakeTimers();
		renderPanel();

		act(() => {
			mockState.blobProps?.onShowOriginalStateChange?.("available");
		});
		fireEvent.click(screen.getByRole("button", { name: "Original" }));
		expect(mockState.blobProps?.showOriginalRequestId).toBe(1);

		act(() => {
			mockState.blobProps?.onShowOriginalStateChange?.("loading");
		});
		const loadingButton = screen.getByRole("button", { name: "Original" });
		expect(loadingButton).toBeDisabled();
		expect(loadingButton.querySelector("svg")).toHaveClass("animate-spin");

		act(() => {
			mockState.blobProps?.onShowOriginalStateChange?.("success");
		});
		expect(screen.getByRole("button", { name: "Original" })).toBeDisabled();

		act(() => {
			vi.advanceTimersByTime(650);
		});
		const collapsedSegment = screen
			.getByRole("button", { name: "Original" })
			.closest("div")?.parentElement;
		expect(collapsedSegment).toHaveClass(
			"max-w-0",
			"translate-x-2",
			"opacity-0",
		);

		act(() => {
			vi.advanceTimersByTime(220);
		});
		expect(
			screen.queryByRole("button", { name: "Original" }),
		).not.toBeInTheDocument();
	});

	it("keeps the original button visible again when success is followed by availability", () => {
		vi.useFakeTimers();
		renderPanel();

		act(() => {
			mockState.blobProps?.onShowOriginalStateChange?.("success");
			vi.advanceTimersByTime(650);
			mockState.blobProps?.onShowOriginalStateChange?.("available");
		});

		expect(screen.getByRole("button", { name: "Original" })).toBeEnabled();
	});
});

function getGestureSurface() {
	const surface = screen.getByTestId("panel-preview-viewport").parentElement;
	if (!surface) {
		throw new Error("Image gesture surface not found");
	}
	return surface;
}

function mockImageGeometry() {
	const viewport = screen.getByTestId("panel-preview-viewport");
	const image = screen.getByTestId("panel-preview-image");
	Object.defineProperty(viewport, "getBoundingClientRect", {
		configurable: true,
		value: () => ({
			bottom: 300,
			height: 300,
			left: 0,
			right: 400,
			top: 0,
			width: 400,
			x: 0,
			y: 0,
			toJSON: () => {},
		}),
	});
	Object.defineProperty(image, "offsetWidth", {
		configurable: true,
		value: 300,
	});
	Object.defineProperty(image, "offsetHeight", {
		configurable: true,
		value: 200,
	});
}
