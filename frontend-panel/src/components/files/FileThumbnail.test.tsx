import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FileThumbnail } from "@/components/files/FileThumbnail";

const mockState = vi.hoisted(() => ({
	enteredViewportRef: vi.fn(),
	useEnteredViewport: vi.fn(),
	useBlobUrl: vi.fn(),
	thumbnailPath: vi.fn((id: number) => `/thumb/${id}`),
}));

vi.mock("@/hooks/useEnteredViewport", () => ({
	useEnteredViewport: (...args: unknown[]) =>
		mockState.useEnteredViewport(...args),
}));

vi.mock("@/hooks/useBlobUrl", () => ({
	useBlobUrl: (...args: unknown[]) => mockState.useBlobUrl(...args),
}));

vi.mock("@/services/fileService", () => ({
	fileService: {
		thumbnailPath: mockState.thumbnailPath,
	},
}));

vi.mock("@/components/files/FileTypeIcon", () => ({
	FileTypeIcon: ({
		mimeType,
		fileName,
		className,
	}: {
		mimeType: string;
		fileName?: string;
		className?: string;
	}) => (
		<span
			data-testid="file-type-icon"
			data-mime-type={mimeType}
			data-file-name={fileName}
			className={className}
		/>
	),
}));

const imageFile = {
	id: 7,
	name: "photo.png",
	mime_type: "image/png",
};

describe("FileThumbnail", () => {
	beforeEach(() => {
		mockState.enteredViewportRef.mockReset();
		mockState.useEnteredViewport.mockReset();
		mockState.useBlobUrl.mockReset();
		mockState.thumbnailPath.mockClear();
		mockState.useEnteredViewport.mockReturnValue({
			ref: mockState.enteredViewportRef,
			hasEnteredViewport: true,
		});
		mockState.useBlobUrl.mockReturnValue({
			blobUrl: null,
			error: false,
			loading: false,
		});
	});

	it("uses the generated thumbnail path for image files", () => {
		mockState.useBlobUrl.mockReturnValue({
			blobUrl: "blob:1",
			error: false,
			loading: false,
		});

		const { container } = render(<FileThumbnail file={imageFile} />);

		expect(mockState.thumbnailPath).toHaveBeenCalledWith(7);
		expect(mockState.useBlobUrl).toHaveBeenCalledWith("/thumb/7", {
			lane: "thumbnail",
		});
		expect(container.querySelector("img")).toHaveAttribute("src", "blob:1");
	});

	it("uses the provided thumbnail path override", () => {
		mockState.useBlobUrl.mockReturnValue({
			blobUrl: null,
			error: false,
			loading: false,
		});

		render(<FileThumbnail file={imageFile} thumbnailPath="/custom-thumb" />);

		expect(mockState.useBlobUrl).toHaveBeenCalledWith("/custom-thumb", {
			lane: "thumbnail",
		});
	});

	it("waits until the thumbnail enters the viewport before queueing the blob fetch", () => {
		mockState.useEnteredViewport.mockReturnValue({
			ref: mockState.enteredViewportRef,
			hasEnteredViewport: false,
		});

		render(<FileThumbnail file={imageFile} />);

		expect(mockState.thumbnailPath).not.toHaveBeenCalled();
		expect(mockState.useBlobUrl).toHaveBeenCalledWith(null, {
			lane: "thumbnail",
		});
		expect(screen.getByTestId("file-type-icon")).toBeInTheDocument();
	});

	it("falls back to the file type icon for non-image files", () => {
		render(
			<FileThumbnail
				file={{ id: 9, name: "report.pdf", mime_type: "application/pdf" }}
			/>,
		);

		expect(mockState.useBlobUrl).toHaveBeenCalledWith(null, {
			lane: "thumbnail",
		});
		expect(screen.getByTestId("file-type-icon")).toHaveAttribute(
			"data-mime-type",
			"application/pdf",
		);
	});

	it("falls back to the icon when blob loading fails or is absent", () => {
		mockState.useBlobUrl.mockReturnValue({
			blobUrl: null,
			error: true,
			loading: false,
		});

		const { rerender } = render(<FileThumbnail file={imageFile} size="lg" />);
		expect(screen.getByTestId("file-type-icon")).toHaveClass("h-12", "w-12");

		mockState.useBlobUrl.mockReturnValue({
			blobUrl: null,
			error: false,
			loading: false,
		});
		rerender(<FileThumbnail file={imageFile} />);

		expect(screen.getByTestId("file-type-icon")).toHaveClass("h-4", "w-4");
	});

	it("renders medium thumbnails across loading, fallback, and success states", () => {
		mockState.useBlobUrl.mockReturnValue({
			blobUrl: null,
			error: false,
			loading: true,
		});

		const { container, rerender } = render(
			<FileThumbnail file={imageFile} size="md" />,
		);

		expect(screen.getByTestId("thumbnail-loading")).toHaveClass("h-4", "w-4");

		mockState.useBlobUrl.mockReturnValue({
			blobUrl: null,
			error: true,
			loading: false,
		});
		rerender(<FileThumbnail file={imageFile} size="md" />);

		expect(screen.getByTestId("file-type-icon")).toHaveClass("h-5", "w-5");

		mockState.useBlobUrl.mockReturnValue({
			blobUrl: "blob:3",
			error: false,
			loading: false,
		});
		rerender(<FileThumbnail file={imageFile} size="md" />);

		expect(container.querySelector("img")).toHaveAttribute("src", "blob:3");
		expect(container.querySelector("img")).toHaveClass(
			"h-full",
			"w-full",
			"object-cover",
		);
	});

	it("renders a large image preview when a blob URL is available", () => {
		mockState.useBlobUrl.mockReturnValue({
			blobUrl: "blob:2",
			error: false,
			loading: false,
		});

		const { container } = render(<FileThumbnail file={imageFile} size="lg" />);

		expect(container.querySelector("img")).toHaveAttribute("src", "blob:2");
		expect(container.querySelector("img")).toHaveClass(
			"h-full",
			"w-auto",
			"shrink-0",
			"max-w-none",
		);
	});

	it("shows a large spinner while a large image thumbnail is still loading", () => {
		mockState.useBlobUrl.mockReturnValue({
			blobUrl: null,
			error: false,
			loading: true,
		});

		render(<FileThumbnail file={imageFile} size="lg" />);

		expect(screen.getByTestId("thumbnail-loading")).toHaveClass("h-5", "w-5");
		expect(screen.queryByTestId("file-type-icon")).not.toBeInTheDocument();
	});

	it("shows a spinner while an image thumbnail is still loading", () => {
		mockState.useBlobUrl.mockReturnValue({
			blobUrl: null,
			error: false,
			loading: true,
		});

		render(<FileThumbnail file={imageFile} />);

		expect(screen.getByTestId("thumbnail-loading")).toHaveClass("animate-spin");
		expect(screen.queryByTestId("file-type-icon")).not.toBeInTheDocument();
	});
});
