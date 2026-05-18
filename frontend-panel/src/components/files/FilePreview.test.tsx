import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FilePreview } from "@/components/files/FilePreview";

vi.mock("@/components/files/preview/FilePreviewDialog", () => ({
	FilePreviewDialog: ({
		open,
		file,
		downloadPath,
		editable,
		previewLinkFactory,
		mediaStreamLinkFactory,
		wopiSessionFactory,
	}: {
		open: boolean;
		file: { name: string };
		downloadPath?: string;
		editable?: boolean;
		previewLinkFactory?: () => Promise<unknown>;
		mediaStreamLinkFactory?: () => Promise<unknown>;
		wopiSessionFactory?: (appKey: string) => Promise<unknown>;
	}) => (
		<div
			data-testid="preview-dialog"
			data-open={String(open)}
			data-file-name={file.name}
			data-download-path={downloadPath ?? ""}
			data-editable={String(Boolean(editable))}
			data-has-preview-link-factory={String(Boolean(previewLinkFactory))}
			data-has-media-stream-link-factory={String(
				Boolean(mediaStreamLinkFactory),
			)}
			data-has-wopi-session-factory={String(Boolean(wopiSessionFactory))}
		/>
	),
}));

describe("FilePreview", () => {
	it("forwards all props to the preview dialog", () => {
		render(
			<FilePreview
				file={{ id: 7, name: "report.pdf" } as never}
				open
				onClose={vi.fn()}
				onFileUpdated={vi.fn()}
				downloadPath="/files/7/download"
				editable
				previewLinkFactory={async () => ({})}
				mediaStreamLinkFactory={async () => ({})}
				wopiSessionFactory={async () => ({})}
			/>,
		);

		expect(screen.getByTestId("preview-dialog")).toHaveAttribute(
			"data-open",
			"true",
		);
		expect(screen.getByTestId("preview-dialog")).toHaveAttribute(
			"data-file-name",
			"report.pdf",
		);
		expect(screen.getByTestId("preview-dialog")).toHaveAttribute(
			"data-download-path",
			"/files/7/download",
		);
		expect(screen.getByTestId("preview-dialog")).toHaveAttribute(
			"data-editable",
			"true",
		);
		expect(screen.getByTestId("preview-dialog")).toHaveAttribute(
			"data-has-preview-link-factory",
			"true",
		);
		expect(screen.getByTestId("preview-dialog")).toHaveAttribute(
			"data-has-media-stream-link-factory",
			"true",
		);
		expect(screen.getByTestId("preview-dialog")).toHaveAttribute(
			"data-has-wopi-session-factory",
			"true",
		);
	});
});
