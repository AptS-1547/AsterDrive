import { fireEvent, render, screen } from "@testing-library/react";
import { cloneElement, isValidElement } from "react";
import { describe, expect, it, vi } from "vitest";
import { UsersTable } from "@/components/admin/admin-users-page/UsersTable";
import type { UserInfo } from "@/types/api";

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
	}),
}));

vi.mock("@/components/common/AdminTable", () => ({
	ADMIN_INTERACTIVE_TABLE_ROW_CLASS: "interactive-row",
	ADMIN_TABLE_BADGE_CELL_CLASS: "badge-cell",
	ADMIN_TABLE_MONO_TEXT_CLASS: "mono-cell",
	ADMIN_TABLE_STACKED_CELL_CLASS: "stacked-cell",
	ADMIN_TABLE_TEXT_CELL_CLASS: "text-cell",
	AdminSortableTableHead: ({ children }: { children: React.ReactNode }) => (
		<th>{children}</th>
	),
	AdminTable: ({ children }: { children: React.ReactNode }) => (
		<table>{children}</table>
	),
	AdminTableBody: ({ children }: { children: React.ReactNode }) => (
		<tbody>{children}</tbody>
	),
	AdminTableCell: ({
		children,
		className,
		onClick,
		onKeyDown,
	}: {
		children: React.ReactNode;
		className?: string;
		onClick?: (event: React.MouseEvent<HTMLTableCellElement>) => void;
		onKeyDown?: (event: React.KeyboardEvent<HTMLTableCellElement>) => void;
	}) => (
		<td className={className} onClick={onClick} onKeyDown={onKeyDown}>
			{children}
		</td>
	),
	AdminTableHead: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => <th className={className}>{children}</th>,
	AdminTableHeader: ({ children }: { children: React.ReactNode }) => (
		<thead>{children}</thead>
	),
	AdminTableRow: ({
		children,
		className,
		onClick,
		onKeyDown,
		tabIndex,
	}: {
		children: React.ReactNode;
		className?: string;
		onClick?: () => void;
		onKeyDown?: (event: React.KeyboardEvent<HTMLTableRowElement>) => void;
		tabIndex?: number;
	}) => (
		<tr
			className={className}
			onClick={onClick}
			onKeyDown={onKeyDown}
			tabIndex={tabIndex}
		>
			{children}
		</tr>
	),
	AdminTableShell: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/components/common/UserAvatarImage", () => ({
	UserAvatarImage: ({ name }: { name: string }) => (
		<span aria-hidden="true">{name.slice(0, 1)}</span>
	),
}));

vi.mock("@/components/common/userBadgeClasses", () => ({
	getRoleBadgeClass: () => "role-badge",
	getStatusBadgeClass: () => "status-badge",
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
		type,
		...props
	}: {
		children?: React.ReactNode;
		disabled?: boolean;
		onClick?: () => void;
		type?: "button" | "submit";
		[key: string]: unknown;
	}) => (
		<button
			type={type ?? "button"}
			disabled={disabled}
			onClick={onClick}
			{...props}
		>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => (
		<span aria-hidden="true" data-icon-name={name} />
	),
}));

vi.mock("@/components/ui/progress", () => ({
	Progress: ({ value }: { value: number }) => (
		<span>{`progress:${value}`}</span>
	),
}));

vi.mock("@/components/ui/tooltip", () => ({
	Tooltip: ({ children }: { children: React.ReactNode }) => children,
	TooltipContent: ({ children }: { children: React.ReactNode }) => (
		<div role="tooltip">{children}</div>
	),
	TooltipProvider: ({ children }: { children: React.ReactNode }) => children,
	TooltipTrigger: ({
		children,
		render,
	}: {
		children?: React.ReactNode;
		render?: React.ReactNode;
	}) => {
		if (render && isValidElement(render)) {
			return cloneElement(render, undefined, children);
		}

		return <>{render ?? children}</>;
	},
}));

vi.mock("@/lib/format", () => ({
	formatBytes: (value: number) => `${value} B`,
}));

const user = (overrides: Partial<UserInfo> = {}): UserInfo => ({
	created_at: "2026-03-28T00:00:00Z",
	email: "alice@example.com",
	email_verified: true,
	id: 11,
	pending_email: null,
	policy_group_id: null,
	profile: {
		avatar: {
			source: "none",
			url_512: null,
			url_1024: null,
			version: 0,
		},
		display_name: null,
	},
	role: "user",
	status: "active",
	storage_quota: 10 * 1024 * 1024,
	storage_used: 5 * 1024 * 1024,
	updated_at: "2026-03-28T00:00:00Z",
	username: "alice",
	...overrides,
});

function renderTable(
	props: Partial<React.ComponentProps<typeof UsersTable>> = {},
) {
	const defaultProps: React.ComponentProps<typeof UsersTable> = {
		deletingUserId: null,
		onDeleteUser: vi.fn(),
		onOpenUserDetail: vi.fn(),
		onSortChange: vi.fn(),
		sortBy: "id",
		sortOrder: "asc",
		users: [user()],
	};

	return render(<UsersTable {...defaultProps} {...props} />);
}

describe("UsersTable", () => {
	it("keeps the protected initial admin delete action accessible in a fixed trigger", () => {
		const onDeleteUser = vi.fn();

		renderTable({
			onDeleteUser,
			users: [user({ id: 1, username: "root" })],
		});

		const deleteButton = screen.getByRole("button", { name: "delete_user" });

		expect(deleteButton).toBeDisabled();
		expect(deleteButton.parentElement).toHaveClass(
			"inline-flex",
			"size-8",
			"shrink-0",
		);
		expect(screen.getByRole("tooltip")).toHaveTextContent(
			"initial_admin_delete_blocked",
		);

		fireEvent.click(deleteButton);

		expect(onDeleteUser).not.toHaveBeenCalled();
	});

	it("runs delete actions without opening the row detail", () => {
		const onDeleteUser = vi.fn();
		const onOpenUserDetail = vi.fn();

		renderTable({ onDeleteUser, onOpenUserDetail });

		fireEvent.click(screen.getByRole("button", { name: "delete_user" }));

		expect(onDeleteUser).toHaveBeenCalledWith(11);
		expect(onOpenUserDetail).not.toHaveBeenCalled();

		const row = screen.getByText("alice").closest("tr");
		expect(row).not.toBeNull();
		fireEvent.click(row as HTMLElement);

		expect(onOpenUserDetail).toHaveBeenCalledWith(11);
	});
});
