import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DRAG_MIME } from "@/lib/constants";
import {
	getInvalidInternalDropReason,
	hasInternalDragData,
	readInternalDragData,
	setInternalDragPreview,
	writeInternalDragData,
} from "@/lib/dragDrop";

describe("dragDrop", () => {
	let requestAnimationFrameSpy: ReturnType<typeof vi.spyOn>;
	let canvasContextSpy: ReturnType<typeof vi.spyOn>;

	beforeEach(() => {
		document.body.innerHTML = "";
		requestAnimationFrameSpy = vi
			.spyOn(window, "requestAnimationFrame")
			.mockImplementation(() => 1);
		canvasContextSpy = vi
			.spyOn(HTMLCanvasElement.prototype, "getContext")
			.mockReturnValue({
				drawImage: vi.fn(),
			} as unknown as CanvasRenderingContext2D);
	});

	afterEach(() => {
		requestAnimationFrameSpy.mockRestore();
		canvasContextSpy.mockRestore();
		document.body.innerHTML = "";
	});

	it("detects whether a data transfer contains internal drag data", () => {
		expect(hasInternalDragData(null)).toBe(false);
		expect(
			hasInternalDragData({
				types: ["text/plain", DRAG_MIME],
			} as unknown as DataTransfer),
		).toBe(true);
		expect(
			hasInternalDragData({
				types: ["text/plain"],
			} as unknown as DataTransfer),
		).toBe(false);
	});

	it("reads and sanitizes internal drag payloads", () => {
		const dataTransfer = {
			types: [DRAG_MIME],
			getData: vi.fn().mockReturnValue(
				JSON.stringify({
					fileIds: [7, -1, 0, 3.2, 8],
					folderIds: [4, "6", null, 9],
				}),
			),
		} as unknown as DataTransfer;

		expect(readInternalDragData(dataTransfer)).toEqual({
			fileIds: [7, 8],
			folderIds: [4, 9],
		});
	});

	it("returns null for invalid or empty internal drag payloads", () => {
		expect(
			readInternalDragData({
				types: [DRAG_MIME],
				getData: vi.fn().mockReturnValue(""),
			} as unknown as DataTransfer),
		).toBeNull();
		expect(
			readInternalDragData({
				types: [DRAG_MIME],
				getData: vi.fn().mockReturnValue("{bad json"),
			} as unknown as DataTransfer),
		).toBeNull();
		expect(
			readInternalDragData({
				types: [DRAG_MIME],
				getData: vi.fn().mockReturnValue(
					JSON.stringify({
						fileIds: [-1, 0],
						folderIds: ["x"],
					}),
				),
			} as unknown as DataTransfer),
		).toBeNull();
	});

	it("writes the expected MIME payload and move effect", () => {
		const setData = vi.fn();
		const dataTransfer = {
			effectAllowed: "copy",
			setData,
		} as unknown as DataTransfer;

		writeInternalDragData(dataTransfer, {
			fileIds: [7, 8],
			folderIds: [3],
		});

		expect(setData).toHaveBeenCalledWith(
			DRAG_MIME,
			JSON.stringify({
				fileIds: [7, 8],
				folderIds: [3],
			}),
		);
		expect(dataTransfer.effectAllowed).toBe("move");
	});

	it("identifies invalid self and descendant drops", () => {
		const dragData = { fileIds: [10], folderIds: [4, 7] };

		expect(getInvalidInternalDropReason(dragData, 7, [1, 2, 3])).toBe("self");
		expect(getInvalidInternalDropReason(dragData, 9, [1, 4, 9])).toBe(
			"descendant",
		);
		expect(getInvalidInternalDropReason(dragData, null, [1, 2, 3])).toBeNull();
	});

	it("forces cloned drag-preview images to render eagerly", () => {
		const source = document.createElement("div");
		source.innerHTML =
			'<img src="blob:thumb-1" loading="lazy" decoding="async">';
		source.getBoundingClientRect = () =>
			({
				width: 120,
				height: 96,
			}) as DOMRect;
		const sourceImage = source.querySelector("img");
		if (!(sourceImage instanceof HTMLImageElement)) {
			throw new Error("source image not found");
		}
		sourceImage.getBoundingClientRect = () =>
			({
				width: 72,
				height: 72,
			}) as DOMRect;
		Object.defineProperty(sourceImage, "complete", { value: false });
		Object.defineProperty(sourceImage, "naturalWidth", { value: 0 });
		Object.defineProperty(sourceImage, "naturalHeight", { value: 0 });
		Object.defineProperty(sourceImage, "currentSrc", { value: "blob:thumb-1" });

		const setDragImage = vi.fn();

		setInternalDragPreview({
			currentTarget: source,
			dataTransfer: {
				setDragImage,
			},
		} as unknown as React.DragEvent<Element>);

		const previewHost = document.body.lastElementChild;
		if (!(previewHost instanceof HTMLElement)) {
			throw new Error("preview host not found");
		}
		const previewImage = previewHost.querySelector("img");
		if (!(previewImage instanceof HTMLImageElement)) {
			throw new Error("preview image not found");
		}

		expect(previewImage.loading).toBe("eager");
		expect(previewImage.decoding).toBe("sync");
		expect(previewImage.draggable).toBe(false);
		expect(previewImage.src).toContain("blob:thumb-1");
		expect(previewHost.querySelector("canvas")).toBeNull();
		expect(setDragImage).toHaveBeenCalledWith(previewHost, 36, 32);
	});

	it("rasterizes loaded images into canvas for drag previews", () => {
		const drawImage = vi.fn();
		canvasContextSpy.mockReturnValue({
			drawImage,
		} as unknown as CanvasRenderingContext2D);

		const source = document.createElement("div");
		source.innerHTML =
			'<img src="blob:thumb-2" loading="lazy" decoding="async">';
		source.getBoundingClientRect = () =>
			({
				width: 120,
				height: 96,
			}) as DOMRect;
		const sourceImage = source.querySelector("img");
		if (!(sourceImage instanceof HTMLImageElement)) {
			throw new Error("source image not found");
		}
		sourceImage.getBoundingClientRect = () =>
			({
				width: 64,
				height: 48,
			}) as DOMRect;
		Object.defineProperty(sourceImage, "complete", { value: true });
		Object.defineProperty(sourceImage, "naturalWidth", { value: 512 });
		Object.defineProperty(sourceImage, "naturalHeight", { value: 384 });
		Object.defineProperty(sourceImage, "currentSrc", { value: "blob:thumb-2" });

		const setDragImage = vi.fn();

		setInternalDragPreview({
			currentTarget: source,
			dataTransfer: {
				setDragImage,
			},
		} as unknown as React.DragEvent<Element>);

		const previewHost = document.body.lastElementChild;
		if (!(previewHost instanceof HTMLElement)) {
			throw new Error("preview host not found");
		}

		expect(previewHost.querySelector("img")).toBeNull();
		expect(previewHost.querySelector("canvas")).toBeInTheDocument();
		expect(drawImage).toHaveBeenCalledWith(sourceImage, 0, 0, 64, 48);
		expect(setDragImage).toHaveBeenCalledWith(previewHost, 36, 32);
	});
});
