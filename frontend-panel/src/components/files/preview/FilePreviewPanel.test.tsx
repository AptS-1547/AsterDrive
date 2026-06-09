import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FilePreviewPanel } from "@/components/files/preview/FilePreviewPanel";

vi.mock("@/components/files/FileThumbnail", () => ({
	FileThumbnail: ({ file }: { file: { name: string } }) => (
		<div data-testid="thumbnail">{file.name}</div>
	),
}));

vi.mock("@/components/ui/dialog", () => ({
	DialogHeader: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DialogTitle: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => <h2 className={className}>{children}</h2>,
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		disabled,
		onClick,
		title,
	}: {
		children: React.ReactNode;
		disabled?: boolean;
		onClick?: () => void;
		title?: string;
	}) => (
		<button type="button" disabled={disabled} onClick={onClick} title={title}>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{name}</span>,
}));

vi.mock("@/components/ui/scroll-area", () => ({
	ScrollArea: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

describe("FilePreviewPanel", () => {
	it("renders the file name in the dialog title", () => {
		render(
			<FilePreviewPanel
				file={{
					id: 7,
					name: "notes.ts",
					mime_type: "text/typescript",
					size: 128,
				}}
				body={<div>preview body</div>}
				allOptionsCount={1}
				usesInnerScroll={false}
				fillsViewportHeight={false}
				isExpanded={false}
				isDirty={false}
				onChooseOpenMethod={vi.fn()}
				onToggleExpand={vi.fn()}
				onClose={vi.fn()}
				chooseOpenMethodLabel="Choose app"
				enterFullscreenLabel="Enter fullscreen"
				exitFullscreenLabel="Exit fullscreen"
				closeLabel="Close"
			/>,
		);

		expect(
			screen.getByRole("heading", { name: "notes.ts" }),
		).toBeInTheDocument();
	});
});
