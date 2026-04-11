import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FileGrid } from "@/components/files/FileGrid";

const mockState = vi.hoisted(() => ({
	store: {
		selectedFileIds: new Set<number>(),
		selectedFolderIds: new Set<number>(),
		selectOnlyFile: vi.fn(),
		selectOnlyFolder: vi.fn(),
		toggleFileSelection: vi.fn(),
		toggleFolderSelection: vi.fn(),
	},
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => `translated:${key}`,
	}),
}));

vi.mock("@/stores/fileStore", () => ({
	useFileStore: (selector: (state: typeof mockState.store) => unknown) =>
		selector(mockState.store),
}));

vi.mock("@/components/files/FileContextMenu", () => ({
	FileContextMenu: ({
		children,
		onPageShare,
		onDirectShare,
		onCopy,
		onMove,
		onRename,
		onToggleLock,
		onDelete,
		onVersions,
		onInfo,
		onDownload,
	}: {
		children: React.ReactNode;
		onPageShare?: () => void;
		onDirectShare?: () => void;
		onCopy?: () => void;
		onMove?: () => void;
		onRename?: () => void;
		onToggleLock?: () => void;
		onDelete?: () => void;
		onVersions?: () => void;
		onInfo?: () => void;
		onDownload?: () => void;
	}) => (
		<div>
			{children}
			{onPageShare && (
				<button type="button" onClick={onPageShare}>
					share-page
				</button>
			)}
			{onDirectShare && (
				<button type="button" onClick={onDirectShare}>
					share-direct
				</button>
			)}
			{onCopy && (
				<button type="button" onClick={onCopy}>
					copy
				</button>
			)}
			{onMove && (
				<button type="button" onClick={onMove}>
					move
				</button>
			)}
			{onRename && (
				<button type="button" onClick={onRename}>
					rename
				</button>
			)}
			{onToggleLock && (
				<button type="button" onClick={onToggleLock}>
					lock
				</button>
			)}
			{onDelete && (
				<button type="button" onClick={onDelete}>
					delete
				</button>
			)}
			{onVersions && (
				<button type="button" onClick={onVersions}>
					versions
				</button>
			)}
			{onInfo && (
				<button type="button" onClick={onInfo}>
					info
				</button>
			)}
			{onDownload && (
				<button type="button" onClick={onDownload}>
					download
				</button>
			)}
		</div>
	),
}));

vi.mock("@/components/files/FileCard", () => ({
	FileCard: ({
		item,
		isFolder,
		selected,
		onSelect,
		onClick,
		onDoubleClick,
		dragData,
		targetPathIds,
		fading,
	}: {
		item: { name: string };
		isFolder: boolean;
		selected: boolean;
		onSelect: () => void;
		onClick: () => void;
		onDoubleClick?: () => void;
		dragData?: { fileIds: number[]; folderIds: number[] };
		targetPathIds?: number[];
		fading?: boolean;
	}) => (
		<div
			data-testid={isFolder ? "folder-card" : "file-card"}
			data-selected={String(selected)}
			data-drag-file-ids={dragData?.fileIds.join(",") ?? ""}
			data-drag-folder-ids={dragData?.folderIds.join(",") ?? ""}
			data-target-path-ids={targetPathIds?.join(",") ?? ""}
			data-fading={String(Boolean(fading))}
		>
			<button type="button" onClick={onClick}>
				open:{item.name}
			</button>
			<button type="button" onClick={onDoubleClick}>
				open-double:{item.name}
			</button>
			<button type="button" onClick={onSelect}>
				select:{item.name}
			</button>
		</div>
	),
}));

describe("FileGrid", () => {
	beforeEach(() => {
		mockState.store.selectedFileIds = new Set();
		mockState.store.selectedFolderIds = new Set();
		mockState.store.selectOnlyFile.mockReset();
		mockState.store.selectOnlyFolder.mockReset();
		mockState.store.toggleFileSelection.mockReset();
		mockState.store.toggleFolderSelection.mockReset();
	});

	it("renders section headers and computed drag metadata", () => {
		mockState.store.selectedFileIds = new Set([2, 3]);
		mockState.store.selectedFolderIds = new Set([1]);

		render(
			<FileGrid
				folders={[{ id: 1, name: "Docs" } as never]}
				files={[{ id: 2, name: "report.pdf" } as never]}
				browserOpenMode="single_click"
				breadcrumbPathIds={[10, 11]}
				onFolderOpen={vi.fn()}
				onFileClick={vi.fn()}
				onShare={vi.fn()}
				onDownload={vi.fn()}
				onCopy={vi.fn()}
				onToggleLock={vi.fn()}
				onDelete={vi.fn()}
				fadingFileIds={new Set([2])}
				fadingFolderIds={new Set([1])}
			/>,
		);

		expect(screen.getByText("translated:folders_section")).toBeInTheDocument();
		expect(screen.getByText("translated:files_section")).toBeInTheDocument();
		expect(screen.getByTestId("folder-card")).toHaveAttribute(
			"data-selected",
			"true",
		);
		expect(screen.getByTestId("folder-card")).toHaveAttribute(
			"data-drag-folder-ids",
			"1",
		);
		expect(screen.getByTestId("folder-card")).toHaveAttribute(
			"data-target-path-ids",
			"10,11,1",
		);
		expect(screen.getByTestId("folder-card")).toHaveAttribute(
			"data-fading",
			"true",
		);
		expect(screen.getByTestId("file-card")).toHaveAttribute(
			"data-drag-file-ids",
			"2,3",
		);
		expect(screen.getByTestId("file-card")).toHaveAttribute(
			"data-drag-folder-ids",
			"1",
		);
		expect(screen.getByTestId("file-card")).toHaveAttribute(
			"data-fading",
			"true",
		);
	});

	it("wires folder and file click and selection handlers", () => {
		const onFolderOpen = vi.fn();
		const onFileClick = vi.fn();

		render(
			<FileGrid
				folders={[{ id: 1, name: "Docs" } as never]}
				files={[{ id: 2, name: "report.pdf" } as never]}
				browserOpenMode="single_click"
				onFolderOpen={onFolderOpen}
				onFileClick={onFileClick}
				onShare={vi.fn()}
				onDownload={vi.fn()}
				onCopy={vi.fn()}
				onToggleLock={vi.fn()}
				onDelete={vi.fn()}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "open:Docs" }));
		fireEvent.click(screen.getByRole("button", { name: "select:Docs" }));
		fireEvent.click(screen.getByRole("button", { name: "open:report.pdf" }));
		fireEvent.click(screen.getByRole("button", { name: "select:report.pdf" }));

		expect(onFolderOpen).toHaveBeenCalledWith(1, "Docs");
		expect(mockState.store.toggleFolderSelection).toHaveBeenCalledWith(1);
		expect(onFileClick).toHaveBeenCalledWith(
			expect.objectContaining({ id: 2 }),
		);
		expect(mockState.store.toggleFileSelection).toHaveBeenCalledWith(2);
	});

	it("selects folders and files on single click and opens them on double click in double-click mode", () => {
		const onFolderOpen = vi.fn();
		const onFileClick = vi.fn();

		render(
			<FileGrid
				folders={[{ id: 1, name: "Docs" } as never]}
				files={[{ id: 2, name: "report.pdf" } as never]}
				browserOpenMode="double_click"
				onFolderOpen={onFolderOpen}
				onFileClick={onFileClick}
				onShare={vi.fn()}
				onDownload={vi.fn()}
				onCopy={vi.fn()}
				onToggleLock={vi.fn()}
				onDelete={vi.fn()}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "open:Docs" }));
		fireEvent.click(screen.getByRole("button", { name: "open:report.pdf" }));
		fireEvent.click(screen.getByRole("button", { name: "open-double:Docs" }));
		fireEvent.click(
			screen.getByRole("button", { name: "open-double:report.pdf" }),
		);

		expect(mockState.store.selectOnlyFolder).toHaveBeenCalledWith(1);
		expect(mockState.store.selectOnlyFile).toHaveBeenCalledWith(2);
		expect(onFolderOpen).toHaveBeenCalledWith(1, "Docs");
		expect(onFileClick).toHaveBeenCalledWith(
			expect.objectContaining({ id: 2 }),
		);
	});

	it("maps context menu actions to the expected callbacks", () => {
		const onShare = vi.fn();
		const onDownload = vi.fn();
		const onCopy = vi.fn();
		const onMove = vi.fn();
		const onToggleLock = vi.fn();
		const onDelete = vi.fn();
		const onRename = vi.fn();
		const onVersions = vi.fn();
		const onInfo = vi.fn();

		render(
			<FileGrid
				folders={[{ id: 1, name: "Docs", is_locked: false } as never]}
				files={[{ id: 2, name: "report.pdf", is_locked: true } as never]}
				browserOpenMode="single_click"
				onFolderOpen={vi.fn()}
				onFileClick={vi.fn()}
				onShare={onShare}
				onDownload={onDownload}
				onCopy={onCopy}
				onMove={onMove}
				onToggleLock={onToggleLock}
				onDelete={onDelete}
				onRename={onRename}
				onVersions={onVersions}
				onInfo={onInfo}
			/>,
		);

		const buttons = screen.getAllByRole("button", {
			name: /share-page|share-direct|copy|move|rename|lock|delete|versions|info|download/i,
		});
		for (const button of buttons) {
			fireEvent.click(button);
		}

		expect(onShare).toHaveBeenNthCalledWith(1, {
			folderId: 1,
			name: "Docs",
			initialMode: "page",
		});
		expect(onShare).toHaveBeenNthCalledWith(2, {
			fileId: 2,
			name: "report.pdf",
			initialMode: "page",
		});
		expect(onShare).toHaveBeenNthCalledWith(3, {
			fileId: 2,
			name: "report.pdf",
			initialMode: "direct",
		});
		expect(onDownload).toHaveBeenCalledWith(2, "report.pdf");
		expect(onCopy).toHaveBeenNthCalledWith(1, "folder", 1);
		expect(onCopy).toHaveBeenNthCalledWith(2, "file", 2);
		expect(onMove).toHaveBeenNthCalledWith(1, "folder", 1);
		expect(onMove).toHaveBeenNthCalledWith(2, "file", 2);
		expect(onRename).toHaveBeenNthCalledWith(1, "folder", 1, "Docs");
		expect(onRename).toHaveBeenNthCalledWith(2, "file", 2, "report.pdf");
		expect(onToggleLock).toHaveBeenNthCalledWith(1, "folder", 1, false);
		expect(onToggleLock).toHaveBeenNthCalledWith(2, "file", 2, true);
		expect(onDelete).toHaveBeenNthCalledWith(1, "folder", 1);
		expect(onDelete).toHaveBeenNthCalledWith(2, "file", 2);
		expect(onVersions).toHaveBeenCalledWith(2);
		expect(onInfo).toHaveBeenNthCalledWith(1, "folder", 1);
		expect(onInfo).toHaveBeenNthCalledWith(2, "file", 2);
	});
});
