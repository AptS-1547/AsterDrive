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

	it("styles grid-card previews and adds a multi-item badge", () => {
		document.documentElement.style.setProperty("--card", "210 40% 98%");
		document.documentElement.style.setProperty("--muted", "210 16% 92%");
		document.documentElement.style.setProperty("--foreground", "222 47% 11%");
		const source = document.createElement("div");
		source.innerHTML = `
			<div data-drag-preview-hidden>hidden control</div>
			<div data-drag-preview-media>preview media</div>
			<p data-drag-preview-name>Quarterly report.pdf</p>
		`;
		source.getBoundingClientRect = () =>
			({
				width: 180,
				height: 220,
			}) as DOMRect;

		const setDragImage = vi.fn();

		setInternalDragPreview(
			{
				currentTarget: source,
				dataTransfer: {
					setDragImage,
				},
			} as unknown as React.DragEvent<Element>,
			{ itemCount: 3, variant: "grid-card" },
		);

		const previewHost = document.body.lastElementChild;
		if (!(previewHost instanceof HTMLElement)) {
			throw new Error("preview host not found");
		}
		const preview = previewHost.firstElementChild;
		if (!(preview instanceof HTMLElement)) {
			throw new Error("preview not found");
		}

		expect(preview.style.display).toBe("flex");
		expect(preview.style.flexDirection).toBe("column");
		expect(preview.style.background).toBe("rgba(248, 250, 252, 0.86)");
		expect(
			(preview.querySelector("[data-drag-preview-hidden]") as HTMLElement).style
				.display,
		).toBe("none");
		expect(
			(preview.querySelector("[data-drag-preview-media]") as HTMLElement).style
				.height,
		).toBe("5rem");
		expect(
			(preview.querySelector("[data-drag-preview-name]") as HTMLElement).style
				.textOverflow,
		).toBe("ellipsis");
		expect(preview).toHaveTextContent("3 项");
		expect(setDragImage).toHaveBeenCalledWith(previewHost, 36, 32);
	});

	it("styles list-row previews and hides trailing cells", () => {
		const source = document.createElement("tr");
		source.innerHTML = `
			<td><input type="checkbox" /></td>
			<td><div><span>Very long file name.txt</span></div></td>
			<td>size</td>
			<td>date</td>
		`;
		source.getBoundingClientRect = () =>
			({
				width: 640,
				height: 48,
			}) as DOMRect;

		const setDragImage = vi.fn();

		setInternalDragPreview(
			{
				currentTarget: source,
				dataTransfer: {
					setDragImage,
				},
			} as unknown as React.DragEvent<Element>,
			{ itemCount: 2, variant: "list-row" },
			{ x: 4, y: 6 },
		);

		const previewHost = document.body.lastElementChild;
		if (!(previewHost instanceof HTMLElement)) {
			throw new Error("preview host not found");
		}
		const preview = previewHost.firstElementChild;
		if (!(preview instanceof HTMLElement)) {
			throw new Error("preview not found");
		}
		const cells = preview.querySelectorAll("td");
		const nameCell = cells[1] as HTMLElement;
		const label = nameCell.querySelector("span");
		if (!(label instanceof HTMLElement)) {
			throw new Error("name label not found");
		}

		expect(preview.style.display).toBe("flex");
		expect(preview.style.width).toBe("396.8px");
		expect((cells[0] as HTMLElement).style.display).toBe("none");
		expect((cells[2] as HTMLElement).style.display).toBe("none");
		expect(nameCell.style.flex).toBe("1 1 0%");
		expect(label.style.textOverflow).toBe("ellipsis");
		expect(preview).toHaveTextContent("2 项");
		expect(setDragImage).toHaveBeenCalledWith(previewHost, 16, 18);
	});

	it("ignores non-element drag sources", () => {
		const setDragImage = vi.fn();

		setInternalDragPreview({
			currentTarget: document.createTextNode("not an element"),
			dataTransfer: {
				setDragImage,
			},
		} as unknown as React.DragEvent<Element>);

		expect(setDragImage).not.toHaveBeenCalled();
		expect(document.body.lastElementChild).toBeNull();
	});
});
