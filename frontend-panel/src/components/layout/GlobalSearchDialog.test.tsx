import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { GlobalSearchDialog } from "@/components/layout/GlobalSearchDialog";
import type { FileListItem, FolderListItem } from "@/types/api";

const mockState = vi.hoisted(() => ({
	getFile: vi.fn(),
	handleApiError: vi.fn(),
	intersectionCallback: null as IntersectionObserverCallback | null,
	navigate: vi.fn(),
	search: vi.fn(),
	workspace: { kind: "personal" as const },
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, options?: Record<string, unknown>) =>
			options ? `${key}:${JSON.stringify(options)}` : key,
	}),
}));

vi.mock("react-router-dom", () => ({
	useNavigate: () => mockState.navigate,
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: mockState.handleApiError,
}));

vi.mock("@/stores/workspaceStore", () => ({
	useWorkspaceStore: (
		selector: (state: { workspace: typeof mockState.workspace }) => unknown,
	) => selector({ workspace: mockState.workspace }),
}));

vi.mock("@/services/searchService", () => ({
	searchService: {
		search: mockState.search,
	},
}));

vi.mock("@/services/fileService", () => ({
	fileService: {
		getFile: mockState.getFile,
	},
}));

vi.mock("@/components/ui/dialog", () => ({
	Dialog: ({
		open,
		children,
	}: {
		open: boolean;
		onOpenChange: (open: boolean) => void;
		children: ReactNode;
	}) => (open ? <div>{children}</div> : null),
	DialogContent: ({ children }: { children: ReactNode }) => (
		<div>{children}</div>
	),
	DialogHeader: ({ children }: { children: ReactNode }) => (
		<div>{children}</div>
	),
	DialogTitle: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => (
		<span data-testid="icon" data-name={name} />
	),
}));

vi.mock("@/components/files/FileThumbnail", () => ({
	FileThumbnail: () => <span data-testid="file-thumbnail">thumb</span>,
}));

function waitForSearchDebounce() {
	return new Promise((resolve) => window.setTimeout(resolve, 220));
}

describe("GlobalSearchDialog", () => {
	beforeEach(() => {
		mockState.getFile.mockReset();
		mockState.handleApiError.mockReset();
		mockState.intersectionCallback = null;
		mockState.navigate.mockReset();
		mockState.search.mockReset();

		window.IntersectionObserver = class MockIntersectionObserver {
			constructor(callback: IntersectionObserverCallback) {
				mockState.intersectionCallback = callback;
			}

			observe() {}

			disconnect() {}

			unobserve() {}

			takeRecords() {
				return [];
			}

			root = null;
			rootMargin = "";
			thresholds = [];
		} as typeof IntersectionObserver;
	});

	it("debounces searches and renders grouped results", async () => {
		const folder: FolderListItem = {
			id: 3,
			is_locked: false,
			is_shared: false,
			name: "Reports",
			updated_at: "2026-04-15T12:00:00Z",
		};
		const file: FileListItem = {
			id: 7,
			is_locked: false,
			is_shared: false,
			mime_type: "text/plain",
			name: "report.txt",
			size: 2048,
			updated_at: "2026-04-15T12:00:00Z",
		};
		mockState.search.mockResolvedValue({
			files: [file],
			folders: [folder],
			total_files: 1,
			total_folders: 1,
		});

		render(<GlobalSearchDialog open onOpenChange={vi.fn()} />);

		fireEvent.change(screen.getByPlaceholderText("search:placeholder"), {
			target: { value: "report" },
		});
		await waitForSearchDebounce();

		await waitFor(() => {
			expect(mockState.search).toHaveBeenCalledWith(
				{
					q: "report",
					type: "all",
					limit: 10,
				},
				{ signal: expect.any(AbortSignal) },
			);
		});

		expect(await screen.findByText("Reports")).toBeInTheDocument();
		expect(screen.getByText("report.txt")).toBeInTheDocument();
	});

	it("opens file results in their parent folder with preview state", async () => {
		const onOpenChange = vi.fn();
		const file: FileListItem = {
			id: 7,
			is_locked: false,
			is_shared: false,
			mime_type: "text/plain",
			name: "report.txt",
			size: 2048,
			updated_at: "2026-04-15T12:00:00Z",
		};
		mockState.search.mockResolvedValue({
			files: [file],
			folders: [],
			total_files: 1,
			total_folders: 0,
		});
		mockState.getFile.mockResolvedValue({
			folder_id: 42,
		});

		render(<GlobalSearchDialog open onOpenChange={onOpenChange} />);

		fireEvent.change(screen.getByPlaceholderText("search:placeholder"), {
			target: { value: "report" },
		});
		await waitForSearchDebounce();
		expect(await screen.findByText("report.txt")).toBeInTheDocument();

		fireEvent.click(screen.getByText("report.txt"));

		await waitFor(() => {
			expect(mockState.getFile).toHaveBeenCalledWith(7);
		});
		expect(onOpenChange).toHaveBeenCalledWith(false);
		expect(mockState.navigate).toHaveBeenCalledWith("/folder/42", {
			state: {
				searchPreviewFile: file,
			},
			viewTransition: false,
		});
	});

	it("loads more results when the sentinel enters view", async () => {
		const firstPageFile = {
			id: 7,
			is_locked: false,
			is_shared: false,
			mime_type: "text/plain",
			name: "report-1.txt",
			size: 2048,
			updated_at: "2026-04-15T12:00:00Z",
		} satisfies FileListItem;
		const secondPageFile = {
			id: 8,
			is_locked: false,
			is_shared: false,
			mime_type: "text/plain",
			name: "report-2.txt",
			size: 1024,
			updated_at: "2026-04-15T12:00:00Z",
		} satisfies FileListItem;

		mockState.search
			.mockResolvedValueOnce({
				files: [firstPageFile],
				folders: [],
				total_files: 2,
				total_folders: 0,
			})
			.mockResolvedValueOnce({
				files: [secondPageFile],
				folders: [],
				total_files: 2,
				total_folders: 0,
			});

		render(<GlobalSearchDialog open onOpenChange={vi.fn()} />);

		fireEvent.change(screen.getByPlaceholderText("search:placeholder"), {
			target: { value: "report" },
		});
		await waitForSearchDebounce();
		expect(await screen.findByText("report-1.txt")).toBeInTheDocument();
		expect(screen.getByText("report-1.txt")).toBeInTheDocument();

		const loadMoreTarget = document.querySelector("[data-search-load-more]");
		expect(loadMoreTarget).not.toBeNull();
		expect(mockState.intersectionCallback).not.toBeNull();

		mockState.intersectionCallback?.(
			[
				{
					isIntersecting: true,
					target: loadMoreTarget as Element,
				} as IntersectionObserverEntry,
			],
			{} as IntersectionObserver,
		);

		await waitFor(() => {
			expect(mockState.search).toHaveBeenNthCalledWith(
				2,
				{
					q: "report",
					type: "all",
					limit: 10,
					offset: 1,
				},
				{ signal: expect.any(AbortSignal) },
			);
		});
		expect(await screen.findByText("report-2.txt")).toBeInTheDocument();
	});

	it("opens folder results directly with a view transition", async () => {
		const onOpenChange = vi.fn();
		const folder: FolderListItem = {
			id: 3,
			is_locked: false,
			is_shared: false,
			name: "Reports",
			updated_at: "2026-04-15T12:00:00Z",
		};
		mockState.search.mockResolvedValue({
			files: [],
			folders: [folder],
			total_files: 0,
			total_folders: 1,
		});

		render(<GlobalSearchDialog open onOpenChange={onOpenChange} />);

		fireEvent.change(screen.getByPlaceholderText("search:placeholder"), {
			target: { value: "reports" },
		});
		await waitForSearchDebounce();
		expect(await screen.findByText("Reports")).toBeInTheDocument();

		fireEvent.click(screen.getByText("Reports"));

		expect(onOpenChange).toHaveBeenCalledWith(false);
		expect(mockState.navigate).toHaveBeenCalledWith("/folder/3?name=Reports", {
			viewTransition: false,
		});
	});

	it("updates the filter and opens the active file from keyboard navigation", async () => {
		const onOpenChange = vi.fn();
		const folder: FolderListItem = {
			id: 3,
			is_locked: false,
			is_shared: false,
			name: "Reports",
			updated_at: "2026-04-15T12:00:00Z",
		};
		const file: FileListItem = {
			id: 7,
			is_locked: true,
			is_shared: false,
			mime_type: "text/plain",
			name: "report.txt",
			size: 2048,
			updated_at: "2026-04-15T12:00:00Z",
		};
		mockState.search.mockResolvedValue({
			files: [file],
			folders: [folder],
			total_files: 1,
			total_folders: 1,
		});
		mockState.getFile.mockResolvedValue({
			folder_id: 42,
		});

		render(<GlobalSearchDialog open onOpenChange={onOpenChange} />);

		const input = screen.getByPlaceholderText("search:placeholder");
		fireEvent.change(input, {
			target: { value: "report" },
		});
		await waitForSearchDebounce();
		expect(await screen.findByText("report.txt")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "search:files_only" }));
		await waitForSearchDebounce();

		await waitFor(() => {
			expect(mockState.search).toHaveBeenLastCalledWith(
				{
					q: "report",
					type: "file",
					limit: 10,
				},
				{ signal: expect.any(AbortSignal) },
			);
		});

		fireEvent.keyDown(input, { key: "ArrowUp" });
		fireEvent.keyDown(input, { key: "Enter" });

		await waitFor(() => {
			expect(mockState.getFile).toHaveBeenCalledWith(7);
		});
		expect(onOpenChange).toHaveBeenCalledWith(false);
		expect(mockState.navigate).toHaveBeenCalledWith("/folder/42", {
			state: {
				searchPreviewFile: file,
			},
			viewTransition: false,
		});
	});

	it("shows an empty state when a query has no matches", async () => {
		mockState.search.mockResolvedValue({
			files: [],
			folders: [],
			total_files: 0,
			total_folders: 0,
		});

		render(<GlobalSearchDialog open onOpenChange={vi.fn()} />);

		fireEvent.change(screen.getByPlaceholderText("search:placeholder"), {
			target: { value: "unknown" },
		});
		await waitForSearchDebounce();
		await waitFor(() => {
			expect(mockState.search).toHaveBeenCalledTimes(1);
		});

		await waitFor(() => {
			expect(screen.queryByText("search:searching")).not.toBeInTheDocument();
			expect(screen.getByText("search:no_results")).toBeInTheDocument();
		});
	});

	it("shows a search error message when the request fails", async () => {
		mockState.search.mockRejectedValue(new Error("search exploded"));

		render(<GlobalSearchDialog open onOpenChange={vi.fn()} />);

		fireEvent.change(screen.getByPlaceholderText("search:placeholder"), {
			target: { value: "broken" },
		});
		await waitForSearchDebounce();
		await waitFor(() => {
			expect(mockState.search).toHaveBeenCalledTimes(1);
		});

		await waitFor(() => {
			expect(screen.getByText("search:search_error")).toBeInTheDocument();
		});
	});

	it("closes on escape and resets stale results when reopened", async () => {
		const onOpenChange = vi.fn();
		const file: FileListItem = {
			id: 7,
			is_locked: false,
			is_shared: false,
			mime_type: "text/plain",
			name: "report.txt",
			size: 2048,
			updated_at: "2026-04-15T12:00:00Z",
		};
		mockState.search.mockResolvedValue({
			files: [file],
			folders: [],
			total_files: 1,
			total_folders: 0,
		});

		const { rerender } = render(
			<GlobalSearchDialog open onOpenChange={onOpenChange} />,
		);

		const input = screen.getByPlaceholderText("search:placeholder");
		fireEvent.change(input, {
			target: { value: "report" },
		});
		await waitForSearchDebounce();
		expect(await screen.findByText("report.txt")).toBeInTheDocument();

		fireEvent.keyDown(input, { key: "Escape" });
		expect(onOpenChange).toHaveBeenCalledWith(false);

		rerender(<GlobalSearchDialog open={false} onOpenChange={onOpenChange} />);
		rerender(<GlobalSearchDialog open onOpenChange={onOpenChange} />);

		expect(screen.getByPlaceholderText("search:placeholder")).toHaveValue("");
		expect(screen.queryByText("report.txt")).not.toBeInTheDocument();
		expect(screen.getByText("search:start_typing_desc")).toBeInTheDocument();
	});

	it("ignores duplicate file opens while a result is already opening", async () => {
		const file: FileListItem = {
			id: 7,
			is_locked: false,
			is_shared: false,
			mime_type: "text/plain",
			name: "report.txt",
			size: 2048,
			updated_at: "2026-04-15T12:00:00Z",
		};
		let resolveFile: ((value: { folder_id: number }) => void) | undefined;
		mockState.search.mockResolvedValue({
			files: [file],
			folders: [],
			total_files: 1,
			total_folders: 0,
		});
		mockState.getFile.mockReturnValue(
			new Promise((resolve: (value: { folder_id: number }) => void) => {
				resolveFile = resolve;
			}),
		);

		render(<GlobalSearchDialog open onOpenChange={vi.fn()} />);

		fireEvent.change(screen.getByPlaceholderText("search:placeholder"), {
			target: { value: "report" },
		});
		await waitForSearchDebounce();
		expect(await screen.findByText("report.txt")).toBeInTheDocument();

		fireEvent.click(screen.getByText("report.txt"));

		await waitFor(() => {
			expect(mockState.getFile).toHaveBeenCalledTimes(1);
		});
		await waitFor(() => {
			expect(
				screen
					.getAllByTestId("icon")
					.some((icon) => icon.getAttribute("data-name") === "Spinner"),
			).toBe(true);
		});

		fireEvent.click(screen.getByText("report.txt"));
		expect(mockState.getFile).toHaveBeenCalledTimes(1);

		resolveFile?.({ folder_id: 42 });
		await waitFor(() => {
			expect(mockState.navigate).toHaveBeenCalledWith("/folder/42", {
				state: {
					searchPreviewFile: file,
				},
				viewTransition: false,
			});
		});
	});
});
