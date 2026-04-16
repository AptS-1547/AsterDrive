import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { cloneElement, isValidElement } from "react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AdminTasksPage from "@/pages/admin/AdminTasksPage";
import type { TaskInfo } from "@/types/api";

const mockState = vi.hoisted(() => ({
	handleApiError: vi.fn(),
	list: vi.fn(),
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, options?: Record<string, unknown>) => {
			if (key === "admin:entries_page") {
				return `entries:${options?.current}/${options?.pages}/${options?.total}`;
			}
			if (key === "admin:overview_background_tasks_source_system") {
				return "source:system";
			}
			if (key === "admin:overview_background_tasks_source_user") {
				return `source:user:${options?.id}`;
			}
			if (key === "admin:overview_background_tasks_source_team") {
				return `source:team:${options?.id}`;
			}
			if (key === "admin:page_size_option") {
				return `size:${options?.count}`;
			}
			return key;
		},
	}),
}));

vi.mock("@/components/common/EmptyState", () => ({
	EmptyState: ({
		title,
		description,
		icon,
	}: {
		title: string;
		description?: string;
		icon?: React.ReactNode;
	}) => (
		<div>
			<div>{title}</div>
			<div>{description}</div>
			<div>{icon}</div>
		</div>
	),
}));

vi.mock("@/components/common/SkeletonTable", () => ({
	SkeletonTable: ({ columns, rows }: { columns: number; rows: number }) => (
		<div>{`skeleton:${columns}:${rows}`}</div>
	),
}));

vi.mock("@/components/layout/AdminLayout", () => ({
	AdminLayout: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/components/layout/AdminPageHeader", () => ({
	AdminPageHeader: ({
		title,
		description,
		actions,
	}: {
		title: string;
		description: string;
		actions?: React.ReactNode;
	}) => (
		<div>
			<h1>{title}</h1>
			<p>{description}</p>
			<div>{actions}</div>
		</div>
	),
}));

vi.mock("@/components/layout/AdminPageShell", () => ({
	AdminPageShell: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/components/layout/AdminSurface", () => ({
	AdminSurface: ({ children }: { children: React.ReactNode }) => (
		<div data-testid="admin-surface">{children}</div>
	),
}));

vi.mock("@/components/ui/badge", () => ({
	Badge: ({ children }: { children: React.ReactNode }) => (
		<span>{children}</span>
	),
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		disabled,
		onClick,
	}: {
		children: React.ReactNode;
		disabled?: boolean;
		onClick?: () => void;
	}) => (
		<button type="button" disabled={disabled} onClick={onClick}>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{name}</span>,
}));

vi.mock("@/components/ui/scroll-area", () => ({
	ScrollArea: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => <div className={className}>{children}</div>,
}));

vi.mock("@/components/ui/select", () => ({
	Select: ({
		children,
		onValueChange,
		value,
	}: {
		children: React.ReactNode;
		onValueChange?: (value: string) => void;
		value?: string;
	}) => (
		<div>
			<div>{`select:${value}`}</div>
			<button type="button" onClick={() => onValueChange?.("50")}>
				select-50
			</button>
			{children}
		</div>
	),
	SelectContent: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	SelectItem: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	SelectTrigger: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => <div className={className}>{children}</div>,
	SelectValue: () => <span>select-value</span>,
}));

vi.mock("@/components/ui/table", () => ({
	Table: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
	TableBody: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	TableCell: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	TableHead: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	TableHeader: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	TableRow: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/components/ui/tooltip", () => ({
	Tooltip: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	TooltipContent: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	TooltipProvider: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	TooltipTrigger: ({
		children,
		render,
	}: {
		children: React.ReactNode;
		render?: React.ReactElement;
	}) =>
		render && isValidElement(render) ? (
			cloneElement(render, undefined, children)
		) : (
			<div>{children}</div>
		),
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: (...args: unknown[]) => mockState.handleApiError(...args),
}));

vi.mock("@/lib/format", () => ({
	formatDateAbsolute: (value: string) => `date:${value}`,
	formatNumber: (value: number) => String(value),
}));

vi.mock("@/services/adminService", () => ({
	adminTaskService: {
		list: (...args: unknown[]) => mockState.list(...args),
	},
}));

function createTask(overrides: Partial<TaskInfo> = {}): TaskInfo {
	return {
		attempt_count: 0,
		can_retry: false,
		created_at: "2026-04-17T00:00:00Z",
		creator_user_id: 7,
		display_name: "Extract report archive",
		expires_at: "2026-04-18T00:00:00Z",
		finished_at: null,
		id: 31,
		kind: "archive_extract",
		last_error: null,
		max_attempts: 1,
		payload: {
			kind: "archive_extract",
			file_id: 9,
			output_folder_name: "report",
			source_file_name: "report.zip",
			target_folder_id: 2,
		},
		progress_current: 3,
		progress_percent: 60,
		progress_total: 5,
		result: null,
		share_id: null,
		started_at: "2026-04-17T00:01:00Z",
		status: "processing",
		status_text: "extracting entries",
		steps: [],
		team_id: null,
		updated_at: "2026-04-17T00:03:00Z",
		...overrides,
	};
}

function renderPage(initialEntry = "/admin/tasks") {
	render(
		<MemoryRouter initialEntries={[initialEntry]}>
			<AdminTasksPage />
		</MemoryRouter>,
	);
}

describe("AdminTasksPage", () => {
	beforeEach(() => {
		mockState.handleApiError.mockReset();
		mockState.list.mockReset();
		mockState.list.mockResolvedValue({
			items: [createTask()],
			total: 1,
		});
	});

	it("shows a loading skeleton while the task request is pending", () => {
		mockState.list.mockImplementationOnce(() => new Promise(() => undefined));

		renderPage();

		expect(screen.getByText("skeleton:8:6")).toBeInTheDocument();
	});

	it("renders the empty state when there are no recorded tasks", async () => {
		mockState.list.mockResolvedValueOnce({
			items: [],
			total: 0,
		});

		renderPage();

		expect(await screen.findByText("admin:no_tasks")).toBeInTheDocument();
		expect(screen.getByText("admin:no_tasks_desc")).toBeInTheDocument();
		expect(screen.getByText("Clock")).toBeInTheDocument();
	});

	it("renders tasks, paginates, and refreshes the list", async () => {
		mockState.list
			.mockResolvedValueOnce({
				items: [
					createTask(),
					createTask({
						id: 32,
						creator_user_id: null,
						display_name: "Trash cleanup",
						kind: "system_runtime",
						payload: {
							kind: "system_runtime",
							task_name: "trash-cleanup",
						},
						progress_current: 1,
						progress_percent: 100,
						progress_total: 1,
						status: "succeeded",
						status_text: "cleaned up 4 items",
						team_id: null,
						updated_at: "2026-04-17T00:05:00Z",
					}),
				],
				total: 25,
			})
			.mockResolvedValueOnce({
				items: [
					createTask({
						id: 41,
						display_name: "Compress team export",
						kind: "archive_compress",
						last_error: "zip writer failed",
						status: "failed",
						team_id: 8,
						updated_at: "2026-04-17T00:07:00Z",
					}),
				],
				total: 25,
			})
			.mockResolvedValueOnce({
				items: [createTask({ id: 52 })],
				total: 25,
			})
			.mockResolvedValueOnce({
				items: [createTask({ id: 53 })],
				total: 25,
			});

		renderPage();

		await waitFor(() => {
			expect(mockState.list).toHaveBeenNthCalledWith(1, {
				limit: 20,
				offset: 0,
			});
		});
		expect(screen.getByText("Extract report archive")).toBeInTheDocument();
		expect(screen.getByText("Trash cleanup")).toBeInTheDocument();
		expect(screen.getByText("source:user:7")).toBeInTheDocument();
		expect(screen.getByText("source:system")).toBeInTheDocument();
		expect(screen.getByText("date:2026-04-17T00:03:00Z")).toBeInTheDocument();
		expect(screen.getByText("entries:1/2/25")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "CaretRight" }));

		await waitFor(() => {
			expect(mockState.list).toHaveBeenNthCalledWith(2, {
				limit: 20,
				offset: 20,
			});
		});
		expect(screen.getByText("source:team:8")).toBeInTheDocument();
		expect(screen.getByText("zip writer failed")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "select-50" }));

		await waitFor(() => {
			expect(mockState.list).toHaveBeenNthCalledWith(3, {
				limit: 50,
				offset: 0,
			});
		});

		fireEvent.click(screen.getByRole("button", { name: /admin:refresh/i }));

		await waitFor(() => {
			expect(mockState.list).toHaveBeenNthCalledWith(4, {
				limit: 50,
				offset: 0,
			});
		});
	});

	it("routes loading failures through handleApiError", async () => {
		const error = new Error("tasks failed");
		mockState.list.mockRejectedValueOnce(error);

		renderPage();

		await waitFor(() => {
			expect(mockState.handleApiError).toHaveBeenCalledWith(error);
		});
	});
});
