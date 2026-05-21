import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
	type FileBrowserContextValue,
	FileBrowserProvider,
	useFileBrowserContext,
} from "@/components/files/FileBrowserContext";

function Consumer() {
	const context = useFileBrowserContext();
	return (
		<div>
			<span>{context.browserOpenMode}</span>
			<span>{context.files.length}</span>
			<button
				type="button"
				onClick={() =>
					context.onShare({
						fileId: 7,
						initialMode: "direct",
						name: "report.pdf",
					})
				}
			>
				share
			</button>
		</div>
	);
}

function createContextValue(): FileBrowserContextValue {
	return {
		breadcrumbPathIds: [1, 2],
		browserOpenMode: "preview",
		files: [],
		folders: [],
		onCopy: vi.fn(),
		onDelete: vi.fn(),
		onDownload: vi.fn(),
		onFileClick: vi.fn(),
		onFolderOpen: vi.fn(),
		onShare: vi.fn(),
		onToggleLock: vi.fn(),
	};
}

describe("FileBrowserContext", () => {
	it("provides the file browser context value to descendants", () => {
		const value = createContextValue();

		render(
			<FileBrowserProvider value={value}>
				<Consumer />
			</FileBrowserProvider>,
		);

		expect(screen.getByText("preview")).toBeInTheDocument();
		expect(screen.getByText("0")).toBeInTheDocument();
		screen.getByRole("button", { name: "share" }).click();
		expect(value.onShare).toHaveBeenCalledWith({
			fileId: 7,
			initialMode: "direct",
			name: "report.pdf",
		});
	});

	it("throws when consumed outside the provider", () => {
		expect(() => render(<Consumer />)).toThrow(
			"useFileBrowserContext must be used within a FileBrowserProvider",
		);
	});
});
