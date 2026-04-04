import { fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps, ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AdminTeamsPage from "@/pages/admin/AdminTeamsPage";

const mockState = vi.hoisted(() => ({
	handleApiError: vi.fn(),
	listPolicyGroups: vi.fn(),
	reload: vi.fn(),
	toastSuccess: vi.fn(),
}));

const TEAMS = [
	{
		archived_at: null,
		created_at: "2026-04-01T00:00:00Z",
		created_by: 9,
		created_by_username: "root",
		description: "Product and design",
		id: 14,
		member_count: 8,
		name: "Product",
		policy_group_id: 5,
		storage_quota: 0,
		storage_used: 2048,
		updated_at: "2026-04-02T00:00:00Z",
	},
] as const;

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
	}),
}));

vi.mock("sonner", () => ({
	toast: {
		success: (...args: unknown[]) => mockState.toastSuccess(...args),
	},
}));

vi.mock("@/components/admin/AdminTeamDetailDialog", () => ({
	AdminTeamDetailDialog: ({
		open,
		teamId,
	}: {
		open: boolean;
		teamId: number | null;
	}) => (open ? <div>{`detail:${teamId}`}</div> : null),
}));

vi.mock("@/components/common/AdminTableList", () => ({
	AdminTableList: ({
		headerRow,
		items,
		renderRow,
	}: {
		headerRow: ReactNode;
		items: typeof TEAMS;
		renderRow: (item: (typeof TEAMS)[number]) => ReactNode;
	}) => (
		<table>
			{headerRow}
			<tbody>{items.map(renderRow)}</tbody>
		</table>
	),
}));

vi.mock("@/components/layout/AdminLayout", () => ({
	AdminLayout: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}));

vi.mock("@/components/layout/AdminPageHeader", () => ({
	AdminPageHeader: ({
		actions,
		toolbar,
		title,
	}: {
		actions?: ReactNode;
		toolbar?: ReactNode;
		title: string;
	}) => (
		<div>
			<h1>{title}</h1>
			{actions}
			{toolbar}
		</div>
	),
}));

vi.mock("@/components/layout/AdminPageShell", () => ({
	AdminPageShell: ({ children }: { children: ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/components/ui/badge", () => ({
	Badge: ({ children }: { children: ReactNode }) => <span>{children}</span>,
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		onClick,
		type,
	}: {
		children: ReactNode;
		onClick?: () => void;
		type?: "button" | "submit";
	}) => (
		<button type={type ?? "button"} onClick={onClick}>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/dialog", () => ({
	Dialog: ({ children, open }: { children: ReactNode; open: boolean }) =>
		open ? <div>{children}</div> : null,
	DialogContent: ({ children }: { children: ReactNode }) => (
		<div>{children}</div>
	),
	DialogDescription: ({ children }: { children: ReactNode }) => (
		<p>{children}</p>
	),
	DialogFooter: ({ children }: { children: ReactNode }) => (
		<div>{children}</div>
	),
	DialogHeader: ({ children }: { children: ReactNode }) => (
		<div>{children}</div>
	),
	DialogTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{name}</span>,
}));

vi.mock("@/components/ui/input", () => ({
	Input: ({
		onChange,
		value,
	}: {
		onChange?: (event: { target: { value: string } }) => void;
		value?: string;
	}) => (
		<input
			value={value}
			onChange={(event) =>
				onChange?.({ target: { value: event.target.value } })
			}
		/>
	),
}));

vi.mock("@/components/ui/label", () => ({
	Label: ({ children }: { children: ReactNode }) => <span>{children}</span>,
}));

vi.mock("@/components/ui/select", () => ({
	Select: ({ children }: { children: ReactNode }) => <div>{children}</div>,
	SelectContent: ({ children }: { children: ReactNode }) => (
		<div>{children}</div>
	),
	SelectItem: ({ children }: { children: ReactNode }) => <div>{children}</div>,
	SelectTrigger: ({ children }: { children: ReactNode }) => (
		<div>{children}</div>
	),
	SelectValue: ({ children }: { children?: ReactNode }) => <>{children}</>,
}));

vi.mock("@/components/ui/table", () => ({
	TableCell: ({ children, ...props }: ComponentProps<"td">) => (
		<td {...props}>{children}</td>
	),
	TableHead: ({ children, ...props }: ComponentProps<"th">) => (
		<th {...props}>{children}</th>
	),
	TableHeader: ({ children }: { children: ReactNode }) => (
		<thead>{children}</thead>
	),
	TableRow: ({ children, ...props }: ComponentProps<"tr">) => (
		<tr {...props}>{children}</tr>
	),
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: (...args: unknown[]) => mockState.handleApiError(...args),
}));

vi.mock("@/hooks/useApiList", () => ({
	useApiList: () => ({
		items: TEAMS,
		loading: false,
		reload: mockState.reload,
	}),
}));

vi.mock("@/services/adminService", () => ({
	adminPolicyGroupService: {
		listAll: (...args: unknown[]) => mockState.listPolicyGroups(...args),
	},
	adminTeamService: {
		list: vi.fn(),
	},
}));

describe("AdminTeamsPage", () => {
	beforeEach(() => {
		mockState.handleApiError.mockReset();
		mockState.listPolicyGroups.mockReset();
		mockState.reload.mockReset();
		mockState.toastSuccess.mockReset();

		mockState.listPolicyGroups.mockResolvedValue([]);
		mockState.reload.mockResolvedValue(undefined);
	});

	it("opens the team detail dialog when clicking a team row", async () => {
		render(<AdminTeamsPage />);

		fireEvent.click(screen.getByText("Product"));

		expect(screen.getByText("detail:14")).toBeInTheDocument();
	});
});
