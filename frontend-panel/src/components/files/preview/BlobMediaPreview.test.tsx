import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BlobMediaPreview } from "@/components/files/preview/BlobMediaPreview";

const mockState = vi.hoisted(() => ({
	retry: vi.fn(),
	useBlobUrl: vi.fn(),
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
	}),
}));

vi.mock("@/hooks/useBlobUrl", () => ({
	useBlobUrl: (...args: unknown[]) => mockState.useBlobUrl(...args),
}));

const file = { name: "preview.png", mime_type: "image/png" };

describe("BlobMediaPreview", () => {
	beforeEach(() => {
		mockState.retry.mockReset();
		mockState.useBlobUrl.mockReset();
		mockState.useBlobUrl.mockReturnValue({
			blobUrl: "blob:image",
			error: false,
			loading: false,
			retry: mockState.retry,
		});
	});

	it("shows a loading message while the blob is being fetched", () => {
		mockState.useBlobUrl.mockReturnValue({
			blobUrl: null,
			error: false,
			loading: true,
			retry: mockState.retry,
		});

		render(<BlobMediaPreview file={file} mode="image" path="/files/1" />);

		expect(mockState.useBlobUrl).toHaveBeenCalledWith("/files/1");
		expect(screen.getByText("loading_preview")).toBeInTheDocument();
	});

	it("renders the retry state when loading fails", () => {
		mockState.useBlobUrl.mockReturnValue({
			blobUrl: null,
			error: true,
			loading: false,
			retry: mockState.retry,
		});

		render(<BlobMediaPreview file={file} mode="image" path="/files/1" />);

		fireEvent.click(screen.getByRole("button", { name: "preview_retry" }));

		expect(screen.getByText("preview_load_failed")).toBeInTheDocument();
		expect(mockState.retry).toHaveBeenCalledTimes(1);
	});

	it("falls back to the error state when no blob url is available", () => {
		mockState.useBlobUrl.mockReturnValue({
			blobUrl: null,
			error: false,
			loading: false,
			retry: mockState.retry,
		});

		render(<BlobMediaPreview file={file} mode="image" path="/files/1" />);

		expect(screen.getByText("preview_load_failed")).toBeInTheDocument();
	});

	it("renders an image preview with the file name as alt text", () => {
		render(<BlobMediaPreview file={file} mode="image" path="/files/1" />);

		const image = screen.getByRole("img", { name: "preview.png" });

		expect(image).toHaveAttribute("src", "blob:image");
		expect(image).toHaveClass(
			"block",
			"max-h-[min(70vh,48rem)]",
			"max-w-full",
			"object-contain",
		);
		expect(image.parentElement).toHaveClass("mx-auto", "w-fit", "p-4");
	});

	it("renders video and audio previews for their media modes", () => {
		const video = render(
			<BlobMediaPreview
				file={{ name: "clip.mp4", mime_type: "video/mp4" }}
				mode="video"
				path="/files/video"
			/>,
		);

		expect(video.container.querySelector("video")).toHaveAttribute(
			"src",
			"blob:image",
		);
		video.unmount();

		const audio = render(
			<BlobMediaPreview
				file={{ name: "track.mp3", mime_type: "audio/mpeg" }}
				mode="audio"
				path="/files/audio"
			/>,
		);

		expect(audio.container.querySelector("audio")).toHaveAttribute(
			"src",
			"blob:image",
		);
	});
});
