import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cropAvatarImage, renderAvatarCropPreview } from "@/lib/avatarCrop";

function createImageElement({
	height = 100,
	naturalHeight = 400,
	naturalWidth = 800,
	width = 200,
}: {
	height?: number;
	naturalHeight?: number;
	naturalWidth?: number;
	width?: number;
} = {}) {
	const image = document.createElement("img");
	Object.defineProperties(image, {
		height: { configurable: true, value: height },
		naturalHeight: { configurable: true, value: naturalHeight },
		naturalWidth: { configurable: true, value: naturalWidth },
		width: { configurable: true, value: width },
	});
	return image;
}

describe("avatarCrop", () => {
	let clearRect: ReturnType<typeof vi.fn>;
	let drawImage: ReturnType<typeof vi.fn>;
	let getContextSpy: ReturnType<typeof vi.spyOn>;
	let setTransform: ReturnType<typeof vi.fn>;
	let toBlobSpy: ReturnType<typeof vi.spyOn>;

	beforeEach(() => {
		Object.defineProperty(window, "devicePixelRatio", {
			configurable: true,
			value: 1,
		});
		clearRect = vi.fn();
		drawImage = vi.fn();
		setTransform = vi.fn();
		getContextSpy = vi
			.spyOn(HTMLCanvasElement.prototype, "getContext")
			.mockReturnValue({
				clearRect,
				drawImage,
				setTransform,
			} as unknown as CanvasRenderingContext2D);
		toBlobSpy = vi
			.spyOn(HTMLCanvasElement.prototype, "toBlob")
			.mockImplementation((callback: BlobCallback, mimeType?: string) => {
				callback(new Blob(["avatar"], { type: mimeType ?? "image/webp" }));
			});
		vi.spyOn(Date, "now").mockReturnValue(1_779_292_800_000);
	});

	afterEach(() => {
		getContextSpy.mockRestore();
		toBlobSpy.mockRestore();
		vi.restoreAllMocks();
	});

	it("crops the selected source area into a bounded avatar file", async () => {
		const image = createImageElement();
		const file = new File(["source"], "profile.photo.png", {
			type: "image/png",
		});

		const cropped = await cropAvatarImage(
			image,
			file,
			{ height: 60, unit: "px", width: 80, x: 10, y: 5 },
			{
				maxOutputSize: 256,
				outputMimeType: "image/jpeg",
				outputQuality: 0.8,
			},
		);

		expect(clearRect).toHaveBeenCalledWith(0, 0, 256, 256);
		expect(drawImage).toHaveBeenCalledWith(
			image,
			40,
			20,
			320,
			240,
			0,
			0,
			256,
			256,
		);
		expect(toBlobSpy).toHaveBeenCalledWith(
			expect.any(Function),
			"image/jpeg",
			0.8,
		);
		expect(cropped.name).toBe("profile.photo-avatar.jpg");
		expect(cropped.type).toBe("image/jpeg");
		expect(cropped.lastModified).toBe(1_779_292_800_000);
	});

	it("uses webp defaults and falls back to an avatar basename", async () => {
		const image = createImageElement({
			height: 64,
			naturalHeight: 64,
			naturalWidth: 64,
			width: 64,
		});
		const file = new File(["source"], ".png", { type: "image/png" });

		const cropped = await cropAvatarImage(image, file, {
			height: 32,
			unit: "px",
			width: 32,
			x: 0,
			y: 0,
		});

		expect(toBlobSpy).toHaveBeenCalledWith(
			expect.any(Function),
			"image/webp",
			0.92,
		);
		expect(cropped.name).toBe("avatar-avatar.webp");
		expect(cropped.type).toBe("image/webp");
	});

	it("rejects when the canvas cannot export a blob", async () => {
		toBlobSpy.mockImplementationOnce((callback: BlobCallback) => {
			callback(null);
		});

		await expect(
			cropAvatarImage(
				createImageElement(),
				new File(["source"], "profile.png"),
				{ height: 20, unit: "px", width: 20, x: 0, y: 0 },
			),
		).rejects.toThrow("failed to export cropped avatar");
	});

	it("throws when image measurements or canvas context are unavailable", async () => {
		await expect(
			cropAvatarImage(
				createImageElement({ width: 0 }),
				new File(["source"], "profile.png"),
				{ height: 20, unit: "px", width: 20, x: 0, y: 0 },
			),
		).rejects.toThrow("failed to measure the selected image");

		getContextSpy.mockReturnValueOnce(null);

		await expect(
			cropAvatarImage(
				createImageElement(),
				new File(["source"], "profile.png"),
				{ height: 20, unit: "px", width: 20, x: 0, y: 0 },
			),
		).rejects.toThrow("failed to prepare the avatar editor");
	});

	it("renders a high-DPI crop preview without exporting a file", () => {
		Object.defineProperty(window, "devicePixelRatio", {
			configurable: true,
			value: 2,
		});
		const canvas = document.createElement("canvas");
		const image = createImageElement();

		renderAvatarCropPreview(
			image,
			canvas,
			{ height: 25, unit: "px", width: 50, x: 2, y: 4 },
			96,
		);

		expect(canvas.width).toBe(192);
		expect(canvas.height).toBe(192);
		expect(canvas.style.width).toBe("96px");
		expect(canvas.style.height).toBe("96px");
		expect(setTransform).toHaveBeenCalledWith(2, 0, 0, 2, 0, 0);
		expect(drawImage).toHaveBeenCalledWith(
			image,
			8,
			16,
			200,
			100,
			0,
			0,
			96,
			96,
		);
	});

	it("leaves the preview canvas sized when no drawing context exists", () => {
		getContextSpy.mockReturnValueOnce(null);
		const canvas = document.createElement("canvas");

		renderAvatarCropPreview(
			createImageElement(),
			canvas,
			{ height: 25, unit: "px", width: 25, x: 0, y: 0 },
			80,
		);

		expect(canvas.width).toBe(80);
		expect(canvas.height).toBe(80);
		expect(drawImage).not.toHaveBeenCalled();
	});
});
