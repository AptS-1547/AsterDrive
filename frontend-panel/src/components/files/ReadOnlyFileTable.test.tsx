import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ReadOnlyFileTable } from "@/components/files/ReadOnlyFileTable";

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => `translated:${key}`,
	}),
}));

vi.mock("@/components/files/FileTableCells", () => ({
	FileNameCell: ({
		file,
		thumbnailPath,
	}: {
		file: { name: string };
		thumbnailPath?: string;
	}) => (
		<td data-testid="file-name-cell" data-thumbnail-path={thumbnailPath ?? ""}>
			{file.name}
		</td>
	),
	FolderNameCell: ({ folder }: { folder: { name: string } }) => (
		<td data-testid="folder-name-cell">{folder.name}</td>
	),
	FileSizeCell: ({ size }: { size: number }) => <td>{size}</td>,
	FolderSizeCell: () => <td>folder-size</td>,
	UpdatedAtCell: ({ updatedAt }: { updatedAt: string }) => <td>{updatedAt}</td>,
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		onClick,
	}: {
		children: React.ReactNode;
		onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
	}) => (
		<button type="button" onClick={onClick}>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: () => <span data-testid="icon" />,
}));

vi.mock("@/components/ui/table", () => ({
	Table: ({ children }: { children: React.ReactNode }) => (
		<table>{children}</table>
	),
	TableHeader: ({ children }: { children: React.ReactNode }) => (
		<thead>{children}</thead>
	),
	TableBody: ({ children }: { children: React.ReactNode }) => (
		<tbody>{children}</tbody>
	),
	TableRow: ({
		children,
		onClick,
		className,
	}: {
		children: React.ReactNode;
		onClick?: () => void;
		className?: string;
	}) => (
		<tr data-testid="row" className={className} onClick={onClick}>
			{children}
		</tr>
	),
	TableHead: ({
		children,
		className,
	}: {
		children?: React.ReactNode;
		className?: string;
	}) => <th className={className}>{children}</th>,
	TableCell: ({
		children,
		className,
	}: {
		children?: React.ReactNode;
		className?: string;
	}) => <td className={className}>{children}</td>,
}));

describe("ReadOnlyFileTable", () => {
	it("renders translated headers, spacing wrapper, and table cells", () => {
		const { container } = render(
			<ReadOnlyFileTable
				folders={[{ id: 1, name: "Docs", updated_at: "2026-01-01" } as never]}
				files={[
					{
						id: 2,
						name: "report.pdf",
						size: 123,
						updated_at: "2026-01-02",
					} as never,
				]}
				getThumbnailPath={(file) => `/thumb/${file.id}`}
			/>,
		);

		expect(container.firstChild).toHaveClass("px-4", "md:px-6", "py-2");
		expect(screen.getByText("translated:name")).toBeInTheDocument();
		expect(screen.getByText("translated:size")).toBeInTheDocument();
		expect(screen.getByText("translated:date")).toBeInTheDocument();
		expect(screen.getByTestId("file-name-cell")).toHaveAttribute(
			"data-thumbnail-path",
			"/thumb/2",
		);
		expect(screen.getByTestId("folder-name-cell")).toBeInTheDocument();
	});

	it("invokes folder, file, and download handlers", () => {
		const onFolderClick = vi.fn();
		const onFileClick = vi.fn();
		const onFileDownload = vi.fn();

		render(
			<ReadOnlyFileTable
				folders={[{ id: 1, name: "Docs", updated_at: "2026-01-01" } as never]}
				files={[
					{
						id: 2,
						name: "report.pdf",
						size: 123,
						updated_at: "2026-01-02",
					} as never,
				]}
				onFolderClick={onFolderClick}
				onFileClick={onFileClick}
				onFileDownload={onFileDownload}
			/>,
		);

		const rows = screen.getAllByTestId("row");
		fireEvent.click(rows[1]);
		fireEvent.click(rows[2]);
		fireEvent.click(screen.getByRole("button"));

		expect(onFolderClick).toHaveBeenCalledWith(
			expect.objectContaining({ id: 1 }),
		);
		expect(onFileClick).toHaveBeenCalledWith(
			expect.objectContaining({ id: 2 }),
		);
		expect(onFileDownload).toHaveBeenCalledWith(
			expect.objectContaining({ id: 2 }),
		);
	});

	it("omits the download button when no download handler is provided", () => {
		render(
			<ReadOnlyFileTable
				folders={[]}
				files={[
					{
						id: 2,
						name: "report.pdf",
						size: 123,
						updated_at: "2026-01-02",
					} as never,
				]}
			/>,
		);

		expect(screen.queryByRole("button")).not.toBeInTheDocument();
	});
});
