import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FileInfoDialog } from "@/components/files/FileInfoDialog";

const mockState = vi.hoisted(() => ({
	getFile: vi.fn(),
	getFolderInfo: vi.fn(),
	listFolder: vi.fn(),
}));

function setDesktopMode(matches: boolean) {
	vi.mocked(window.matchMedia).mockImplementation((query: string) => ({
		matches,
		media: query,
		onchange: null,
		addEventListener: vi.fn(),
		removeEventListener: vi.fn(),
		addListener: vi.fn(),
		removeListener: vi.fn(),
		dispatchEvent: vi.fn(),
	}));
}

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, opts?: Record<string, unknown>) => {
			if (key === "info_children_count") {
				return `folders:${opts?.folders} files:${opts?.files}`;
			}
			if (key === "info_loading") {
				return "loading";
			}
			return key;
		},
	}),
}));

vi.mock("@/components/files/FileItemStatusIndicators", () => ({
	FileItemStatusIndicators: ({
		isLocked,
		isShared,
	}: {
		isLocked?: boolean;
		isShared?: boolean;
	}) => (
		<div>{`status:${isLocked ? "locked" : "unlocked"}:${isShared ? "shared" : "private"}`}</div>
	),
}));

vi.mock("@/components/files/FileTypeIcon", () => ({
	FileTypeIcon: ({ fileName }: { fileName?: string }) => (
		<div>{`type:${fileName ?? "unknown"}`}</div>
	),
}));

vi.mock("@/components/ui/dialog", () => ({
	Dialog: ({ children, open }: { children: React.ReactNode; open: boolean }) =>
		open ? <div data-testid="dialog">{children}</div> : null,
	DialogContent: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => <div className={className}>{children}</div>,
	DialogHeader: ({ children, className }: React.ComponentProps<"div">) => (
		<div className={className}>{children}</div>
	),
	DialogTitle: ({ children }: { children: React.ReactNode }) => (
		<h2>{children}</h2>
	),
}));

vi.mock("@/components/ui/scroll-area", () => ({
	ScrollArea: ({ children, className }: React.ComponentProps<"div">) => (
		<div className={className}>{children}</div>
	),
}));

vi.mock("@/lib/format", () => ({
	formatBytes: (value: number) => `bytes:${value}`,
	formatDateAbsolute: (value: string) => `date:${value}`,
}));

vi.mock("@/services/fileService", () => ({
	fileService: {
		getFile: (...args: unknown[]) => mockState.getFile(...args),
		getFolderInfo: (...args: unknown[]) => mockState.getFolderInfo(...args),
		listFolder: (...args: unknown[]) => mockState.listFolder(...args),
	},
}));

describe("FileInfoDialog", () => {
	beforeEach(() => {
		setDesktopMode(false);
		mockState.getFile.mockReset();
		mockState.getFolderInfo.mockReset();
		mockState.listFolder.mockReset();
	});

	it("renders file overview rows without requesting folder counts", () => {
		render(
			<FileInfoDialog
				open
				onOpenChange={vi.fn()}
				file={
					{
						blob_id: 88,
						created_at: "2026-01-01T00:00:00Z",
						id: 1,
						is_locked: true,
						mime_type: "text/markdown",
						name: "notes.md",
						size: 512,
						updated_at: "2026-01-02T00:00:00Z",
					} as never
				}
			/>,
		);

		expect(
			screen.getByRole("heading", { name: "notes.md" }),
		).toBeInTheDocument();
		expect(screen.getAllByText("bytes:512").length).toBe(2);
		expect(screen.getByText("text/markdown")).toBeInTheDocument();
		expect(screen.getByText("date:2026-01-01T00:00:00Z")).toBeInTheDocument();
		expect(screen.getByText("date:2026-01-02T00:00:00Z")).toBeInTheDocument();
		expect(screen.getByText("info_locked_yes")).toBeInTheDocument();
		expect(screen.getByText("status:locked:private")).toBeInTheDocument();
		expect(mockState.getFile).not.toHaveBeenCalled();
		expect(mockState.getFolderInfo).not.toHaveBeenCalled();
		expect(mockState.listFolder).not.toHaveBeenCalled();
	});

	it("loads folder details and child counts when opened and shows the resolved totals", async () => {
		let resolveList:
			| ((value: { files_total: number; folders_total: number }) => void)
			| undefined;

		mockState.getFolderInfo.mockResolvedValueOnce({
			created_at: "2026-02-01T00:00:00Z",
			id: 3,
			is_locked: false,
			name: "Projects",
			policy_id: null,
			updated_at: "2026-02-02T00:00:00Z",
		});
		mockState.listFolder.mockImplementationOnce(
			() =>
				new Promise<{ files_total: number; folders_total: number }>(
					(resolve) => {
						resolveList = resolve;
					},
				),
		);

		render(
			<FileInfoDialog
				open
				onOpenChange={vi.fn()}
				folder={
					{
						id: 3,
						is_locked: false,
						name: "Projects",
						updated_at: "2026-02-02T00:00:00Z",
					} as never
				}
			/>,
		);

		expect(mockState.getFolderInfo).toHaveBeenCalledWith(3);
		expect(mockState.listFolder).toHaveBeenCalledWith(3, {
			file_limit: 0,
			folder_limit: 0,
		});
		expect(screen.getAllByText("loading").length).toBeGreaterThan(0);

		resolveList?.({ files_total: 5, folders_total: 2 });

		expect((await screen.findAllByText("folders:2 files:5")).length).toBe(2);
	});

	it("resets loaded folder counts on close and falls back to loading after a failed refresh", async () => {
		mockState.getFolderInfo
			.mockResolvedValueOnce({
				created_at: "2026-03-01T00:00:00Z",
				id: 9,
				is_locked: true,
				name: "Archive",
				policy_id: 12,
				updated_at: "2026-03-02T00:00:00Z",
			})
			.mockRejectedValueOnce(new Error("unavailable"));
		mockState.listFolder
			.mockResolvedValueOnce({ files_total: 1, folders_total: 4 })
			.mockRejectedValueOnce(new Error("unavailable"));

		const folder = {
			id: 9,
			is_locked: true,
			name: "Archive",
			updated_at: "2026-03-02T00:00:00Z",
		} as never;

		const { rerender } = render(
			<FileInfoDialog open onOpenChange={vi.fn()} folder={folder} />,
		);

		expect((await screen.findAllByText("folders:4 files:1")).length).toBe(2);

		rerender(
			<FileInfoDialog open={false} onOpenChange={vi.fn()} folder={folder} />,
		);
		expect(screen.queryByTestId("dialog")).not.toBeInTheDocument();

		rerender(<FileInfoDialog open onOpenChange={vi.fn()} folder={folder} />);

		await waitFor(() => {
			expect(mockState.getFolderInfo).toHaveBeenCalledTimes(2);
			expect(mockState.listFolder).toHaveBeenCalledTimes(2);
		});
		expect(screen.getAllByText("loading").length).toBeGreaterThan(0);
		expect(screen.queryByText("folders:4 files:1")).not.toBeInTheDocument();
	});

	it("loads file details when opened from a list item", async () => {
		mockState.getFile.mockResolvedValueOnce({
			blob_id: 88,
			created_at: "2026-01-01T00:00:00Z",
			id: 1,
			is_locked: true,
			mime_type: "text/markdown",
			name: "notes.md",
			size: 512,
			updated_at: "2026-01-02T00:00:00Z",
		});

		render(
			<FileInfoDialog
				open
				onOpenChange={vi.fn()}
				file={
					{
						id: 1,
						is_locked: true,
						is_shared: false,
						mime_type: "text/markdown",
						name: "notes.md",
						size: 512,
						updated_at: "2026-01-02T00:00:00Z",
					} as never
				}
			/>,
		);

		expect(mockState.getFile).toHaveBeenCalledWith(1);
		expect(
			await screen.findByText("date:2026-01-01T00:00:00Z"),
		).toBeInTheDocument();
	});

	it("renders a desktop inspector with quick actions and close control", () => {
		setDesktopMode(true);
		const onOpenChange = vi.fn();
		const onPreview = vi.fn();
		const onShare = vi.fn();
		const onRename = vi.fn();

		render(
			<FileInfoDialog
				open
				onOpenChange={onOpenChange}
				onPreview={onPreview}
				onShare={onShare}
				onRename={onRename}
				file={
					{
						blob_id: 77,
						created_at: "2026-03-31T00:00:00Z",
						id: 7,
						is_locked: false,
						is_shared: true,
						mime_type: "application/pdf",
						name: "manual.pdf",
						size: 2048,
						updated_at: "2026-04-01T00:00:00Z",
					} as never
				}
			/>,
		);

		expect(screen.queryByTestId("dialog")).not.toBeInTheDocument();
		expect(screen.getByLabelText("info")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "preview" }));
		expect(onPreview).toHaveBeenCalledWith(
			expect.objectContaining({ id: 7, name: "manual.pdf" }),
		);

		fireEvent.click(screen.getByRole("button", { name: "share" }));
		expect(onShare).toHaveBeenCalledWith({
			fileId: 7,
			name: "manual.pdf",
			initialMode: "page",
		});

		fireEvent.click(screen.getByRole("button", { name: "rename" }));
		expect(onRename).toHaveBeenCalledWith("file", 7, "manual.pdf");

		fireEvent.click(screen.getByRole("button", { name: "close" }));
		expect(onOpenChange).toHaveBeenCalledWith(false);
	});

	it("updates the lock action immediately after toggling", async () => {
		setDesktopMode(true);
		const onToggleLock = vi.fn().mockResolvedValue(true);

		render(
			<FileInfoDialog
				open
				onOpenChange={vi.fn()}
				onToggleLock={onToggleLock}
				file={
					{
						blob_id: 77,
						created_at: "2026-03-31T00:00:00Z",
						id: 7,
						is_locked: false,
						is_shared: false,
						mime_type: "application/pdf",
						name: "manual.pdf",
						size: 2048,
						updated_at: "2026-04-01T00:00:00Z",
					} as never
				}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "lock" }));

		expect(onToggleLock).toHaveBeenCalledWith("file", 7, false);
		expect(
			await screen.findByRole("button", { name: "unlock" }),
		).toBeInTheDocument();
		expect(screen.getByText("info_locked_yes")).toBeInTheDocument();
		expect(screen.getByText("status:locked:private")).toBeInTheDocument();
	});

	it("keeps the desktop inspector mounted long enough to animate out", () => {
		setDesktopMode(true);
		vi.useFakeTimers();

		const file = {
			blob_id: 88,
			created_at: "2026-01-01T00:00:00Z",
			id: 1,
			is_locked: true,
			mime_type: "text/markdown",
			name: "notes.md",
			size: 512,
			updated_at: "2026-01-02T00:00:00Z",
		} as never;

		const { rerender } = render(
			<FileInfoDialog open onOpenChange={vi.fn()} file={file} />,
		);

		rerender(
			<FileInfoDialog open={false} onOpenChange={vi.fn()} file={file} />,
		);
		expect(screen.getByLabelText("info")).toBeInTheDocument();

		act(() => {
			vi.advanceTimersByTime(320);
		});
		expect(screen.queryByLabelText("info")).not.toBeInTheDocument();
	});
});
