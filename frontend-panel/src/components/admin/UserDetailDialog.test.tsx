import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UserDetailDialog } from "@/components/admin/UserDetailDialog";

const mockState = vi.hoisted(() => ({
	assign: vi.fn(),
	handleApiError: vi.fn(),
	listAssignments: vi.fn(),
	listPolicies: vi.fn(),
	onUpdate: vi.fn(),
	remove: vi.fn(),
	revokeSessions: vi.fn(),
	resetPassword: vi.fn(),
	toastError: vi.fn(),
	toastSuccess: vi.fn(),
	updateAssignment: vi.fn(),
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
	}),
}));

vi.mock("sonner", () => ({
	toast: {
		error: (...args: unknown[]) => mockState.toastError(...args),
		success: (...args: unknown[]) => mockState.toastSuccess(...args),
	},
}));

vi.mock("@/components/common/SkeletonTable", () => ({
	SkeletonTable: ({ columns, rows }: { columns: number; rows: number }) => (
		<div>{`skeleton:${columns}:${rows}`}</div>
	),
}));

vi.mock("@/components/common/UserAvatarImage", () => ({
	UserAvatarImage: ({ name }: { name: string }) => (
		<div>{`avatar:${name}`}</div>
	),
}));

vi.mock("@/components/common/UserStatusBadge", () => ({
	getRoleBadgeClass: (role: string) => `role:${role}`,
	getStatusBadgeClass: (status: string) => `status:${status}`,
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
		variant,
	}: {
		children: React.ReactNode;
		disabled?: boolean;
		onClick?: () => void;
		type?: "button" | "submit";
		variant?: string;
	}) => (
		<button
			type={type ?? "button"}
			data-variant={variant}
			disabled={disabled}
			onClick={onClick}
		>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/dialog", () => ({
	Dialog: ({ children, open }: { children: React.ReactNode; open: boolean }) =>
		open ? <div>{children}</div> : null,
	DialogContent: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => <div className={className}>{children}</div>,
	DialogFooter: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DialogHeader: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DialogTitle: ({ children }: { children: React.ReactNode }) => (
		<h2>{children}</h2>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{name}</span>,
}));

vi.mock("@/components/ui/input", () => ({
	Input: ({
		autoComplete,
		className,
		disabled,
		id,
		onChange,
		placeholder,
		readOnly,
		type,
		value,
	}: {
		autoComplete?: string;
		className?: string;
		disabled?: boolean;
		id?: string;
		onChange?: (event: { target: { value: string } }) => void;
		placeholder?: string;
		readOnly?: boolean;
		type?: string;
		value?: string;
	}) => (
		<input
			autoComplete={autoComplete}
			className={className}
			disabled={disabled}
			id={id}
			onChange={(event) =>
				onChange?.({ target: { value: event.target.value } })
			}
			placeholder={placeholder}
			readOnly={readOnly}
			type={type}
			value={value}
		/>
	),
}));

vi.mock("@/components/ui/label", () => ({
	Label: ({
		children,
		htmlFor,
	}: {
		children: React.ReactNode;
		htmlFor?: string;
	}) => <label htmlFor={htmlFor}>{children}</label>,
}));

vi.mock("@/components/ui/progress", () => ({
	Progress: ({ value }: { value: number }) => <div>{`progress:${value}`}</div>,
}));

vi.mock("@/components/ui/select", () => {
	const { createContext, useContext } =
		require("react") as typeof import("react");

	const SelectContext = createContext<{
		disabled?: boolean;
		onValueChange?: (value: string) => void;
	}>({});

	return {
		Select: ({
			children,
			disabled,
			onValueChange,
		}: {
			children: React.ReactNode;
			disabled?: boolean;
			onValueChange?: (value: string) => void;
		}) => (
			<SelectContext.Provider value={{ disabled, onValueChange }}>
				<div>{children}</div>
			</SelectContext.Provider>
		),
		SelectContent: ({ children }: { children: React.ReactNode }) => (
			<div>{children}</div>
		),
		SelectItem: ({
			children,
			value,
		}: {
			children: React.ReactNode;
			value: string;
		}) => {
			const context = useContext(SelectContext);

			return (
				<button
					type="button"
					aria-label={`select-item:${value}`}
					disabled={context.disabled}
					onClick={() => context.onValueChange?.(value)}
				>
					{children}
				</button>
			);
		},
		SelectTrigger: ({
			children,
			className,
		}: {
			children: React.ReactNode;
			className?: string;
		}) => <div className={className}>{children}</div>,
		SelectValue: ({ placeholder }: { placeholder?: string }) => (
			<span>{placeholder ?? "select-value"}</span>
		),
	};
});

vi.mock("@/components/ui/switch", () => ({
	Switch: ({
		checked,
		onCheckedChange,
	}: {
		checked: boolean;
		onCheckedChange?: (checked: boolean) => void;
	}) => (
		<button
			type="button"
			aria-label={`switch:${checked}`}
			onClick={() => onCheckedChange?.(!checked)}
		/>
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
	TooltipTrigger: ({ children }: { children: React.ReactNode }) => children,
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: (...args: unknown[]) => mockState.handleApiError(...args),
}));

vi.mock("@/lib/format", () => ({
	formatBytes: (value: number) => `bytes:${value}`,
	formatDateAbsolute: (value: string) => `date:${value}`,
}));

vi.mock("@/services/adminService", () => ({
	adminUserService: {
		revokeSessions: (...args: unknown[]) => mockState.revokeSessions(...args),
		resetPassword: (...args: unknown[]) => mockState.resetPassword(...args),
	},
	adminPolicyService: {
		list: (...args: unknown[]) => mockState.listPolicies(...args),
	},
	adminUserPolicyService: {
		assign: (...args: unknown[]) => mockState.assign(...args),
		list: (...args: unknown[]) => mockState.listAssignments(...args),
		remove: (...args: unknown[]) => mockState.remove(...args),
		update: (...args: unknown[]) => mockState.updateAssignment(...args),
	},
}));

function createUser(overrides: Record<string, unknown> = {}) {
	return {
		created_at: "2026-03-28T00:00:00Z",
		email: "alice@example.com",
		id: 2,
		profile: {
			avatar: {
				source: "none",
				url_512: null,
				url_1024: null,
				version: 0,
			},
		},
		role: "user",
		status: "active",
		storage_quota: 10 * 1024 * 1024,
		storage_used: 5 * 1024 * 1024,
		updated_at: "2026-03-28T00:00:00Z",
		username: "alice",
		...overrides,
	};
}

function createPolicy(overrides: Record<string, unknown> = {}) {
	return {
		allowed_types: "",
		base_path: "",
		bucket: "",
		chunk_size: 5 * 1024 * 1024,
		created_at: "2026-03-28T00:00:00Z",
		driver_type: "local",
		endpoint: "",
		id: 1,
		is_default: false,
		max_file_size: 0,
		name: "Primary",
		options: "{}",
		updated_at: "2026-03-28T00:00:00Z",
		...overrides,
	};
}

function createAssignment(overrides: Record<string, unknown> = {}) {
	return {
		created_at: "2026-03-28T00:00:00Z",
		id: 101,
		is_default: false,
		policy_id: 1,
		quota_bytes: 0,
		user_id: 2,
		...overrides,
	};
}

function renderDialog(userOverrides: Record<string, unknown> = {}) {
	return render(
		<UserDetailDialog
			user={createUser(userOverrides)}
			open
			onOpenChange={vi.fn()}
			onUpdate={mockState.onUpdate}
		/>,
	);
}

async function waitForPolicyLoad(userId = 2) {
	await waitFor(() => {
		expect(mockState.listAssignments).toHaveBeenCalledWith(userId, {
			limit: 100,
			offset: 0,
		});
	});
}

describe("UserDetailDialog", () => {
	beforeEach(() => {
		mockState.assign.mockReset();
		mockState.handleApiError.mockReset();
		mockState.listAssignments.mockReset();
		mockState.listPolicies.mockReset();
		mockState.onUpdate.mockReset();
		mockState.remove.mockReset();
		mockState.revokeSessions.mockReset();
		mockState.resetPassword.mockReset();
		mockState.toastError.mockReset();
		mockState.toastSuccess.mockReset();
		mockState.updateAssignment.mockReset();

		mockState.assign.mockResolvedValue(undefined);
		mockState.listAssignments.mockResolvedValue({
			items: [createAssignment({ is_default: true })],
			total: 1,
		});
		mockState.listPolicies.mockResolvedValue({
			items: [createPolicy()],
			total: 1,
		});
		mockState.onUpdate.mockResolvedValue(undefined);
		mockState.remove.mockResolvedValue(undefined);
		mockState.revokeSessions.mockResolvedValue(undefined);
		mockState.resetPassword.mockResolvedValue(undefined);
		mockState.updateAssignment.mockResolvedValue(undefined);
	});

	it("loads policies on open and saves changed profile values", async () => {
		renderDialog();

		await waitForPolicyLoad();
		expect(mockState.listPolicies).toHaveBeenCalledWith({
			limit: 100,
			offset: 0,
		});

		fireEvent.click(
			screen.getByRole("button", { name: "select-item:disabled" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "select-item:admin" }));
		fireEvent.change(screen.getByLabelText("quota_mb"), {
			target: { value: "20" },
		});

		fireEvent.click(screen.getByRole("button", { name: /save_changes/i }));

		await waitFor(() => {
			expect(mockState.onUpdate).toHaveBeenCalledWith(2, {
				role: "admin",
				status: "disabled",
				storage_quota: 20 * 1024 * 1024,
			});
		});
	});

	it("assigns policies, updates defaults and quota, blocks default removal, and removes non-default rows", async () => {
		mockState.listAssignments.mockResolvedValue({
			items: [
				createAssignment({
					id: 101,
					is_default: true,
					policy_id: 1,
				}),
				createAssignment({
					id: 102,
					policy_id: 2,
					quota_bytes: 16 * 1024 * 1024,
				}),
			],
			total: 2,
		});
		mockState.listPolicies.mockResolvedValue({
			items: [
				createPolicy({
					id: 1,
					name: "Primary",
				}),
				createPolicy({
					id: 2,
					name: "Archive",
				}),
				createPolicy({
					id: 3,
					driver_type: "s3",
					name: "Cold Backup",
				}),
			],
			total: 3,
		});

		renderDialog();

		await screen.findByText("Primary");
		expect(screen.getByText("Archive")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "select-item:3" }));
		fireEvent.change(
			screen.getByPlaceholderText("quota (MB, 0=core:unlimited)"),
			{ target: { value: "12" } },
		);
		fireEvent.click(screen.getByRole("button", { name: "switch:false" }));
		fireEvent.click(screen.getByRole("button", { name: "core:confirm" }));

		await waitFor(() => {
			expect(mockState.assign).toHaveBeenCalledWith(2, {
				is_default: true,
				policy_id: 3,
				quota_bytes: 12 * 1024 * 1024,
			});
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("policy_assigned");

		fireEvent.click(screen.getByRole("button", { name: "set_default" }));

		await waitFor(() => {
			expect(mockState.updateAssignment).toHaveBeenCalledWith(2, 102, {
				is_default: true,
			});
		});

		fireEvent.click(screen.getAllByRole("button", { name: "edit_quota" })[1]);
		fireEvent.change(screen.getAllByPlaceholderText("0 = core:unlimited")[1], {
			target: { value: "5" },
		});
		fireEvent.click(screen.getByRole("button", { name: "core:save" }));

		await waitFor(() => {
			expect(mockState.updateAssignment).toHaveBeenNthCalledWith(2, 2, 102, {
				quota_bytes: 5 * 1024 * 1024,
			});
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("assignment_updated");

		fireEvent.click(screen.getAllByRole("button", { name: "Trash" })[0]);

		expect(mockState.toastError).toHaveBeenCalledWith(
			"default_policy_remove_blocked",
		);
		expect(mockState.remove).not.toHaveBeenCalled();

		fireEvent.click(screen.getAllByRole("button", { name: "Trash" })[1]);

		await waitFor(() => {
			expect(mockState.remove).toHaveBeenCalledWith(2, 102);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("assignment_removed");
	});

	it("shows initial admin protections and disables role and status changes", async () => {
		mockState.listAssignments.mockResolvedValue({
			items: [],
			total: 0,
		});
		mockState.listPolicies.mockResolvedValue({
			items: [],
			total: 0,
		});

		renderDialog({
			id: 1,
			role: "admin",
			username: "root",
		});

		await waitForPolicyLoad(1);

		expect(screen.getAllByText("initial_admin_protected")).toHaveLength(2);
		expect(
			screen.getByRole("button", { name: "select-item:disabled" }),
		).toBeDisabled();
		expect(
			screen.getByRole("button", { name: "select-item:user" }),
		).toBeDisabled();
		expect(screen.queryByRole("button", { name: /save_changes/i })).toBeNull();
		expect(screen.getByText("no_policies_assigned")).toBeInTheDocument();
	});

	it("caps the dialog height and keeps the detail column scrollable", async () => {
		const { container } = renderDialog();

		await screen.findByText("Primary");

		expect(
			container.querySelector(
				".overflow-hidden.max-h-\\[min\\(880px\\,calc\\(100vh-2rem\\)\\)\\]",
			),
		).not.toBeNull();
		expect(container.querySelector(".overflow-y-auto.p-6")).not.toBeNull();
	});

	it("resets the user's password from the detail dialog", async () => {
		renderDialog();

		await screen.findByText("Primary");
		expect(screen.getByLabelText("password")).toHaveAttribute(
			"autocomplete",
			"new-password",
		);
		expect(screen.getByLabelText("confirm_password")).toHaveAttribute(
			"autocomplete",
			"new-password",
		);

		fireEvent.change(screen.getByLabelText("password"), {
			target: { value: "newsecret456" },
		});
		fireEvent.change(screen.getByLabelText("confirm_password"), {
			target: { value: "newsecret456" },
		});
		fireEvent.click(
			screen.getAllByRole("button", { name: "reset_password" })[0],
		);

		await waitFor(() => {
			expect(mockState.resetPassword).toHaveBeenCalledWith(2, {
				password: "newsecret456",
			});
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith(
			"password_reset_success",
		);
	});

	it("revokes user sessions from the detail dialog", async () => {
		renderDialog();

		await screen.findByText("Primary");

		const revokeButton = screen.getByRole("button", {
			name: /revoke_sessions/i,
		});

		expect(revokeButton).toHaveAttribute("data-variant", "destructive");

		fireEvent.click(revokeButton);

		await waitFor(() => {
			expect(mockState.revokeSessions).toHaveBeenCalledWith(2);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith(
			"revoke_sessions_success",
		);
	});

	it("routes revoke session failures through handleApiError", async () => {
		const error = new Error("revoke failed");
		mockState.revokeSessions.mockRejectedValueOnce(error);

		renderDialog();

		await screen.findByText("Primary");
		fireEvent.click(
			screen.getByRole("button", {
				name: /revoke_sessions/i,
			}),
		);

		await waitFor(() => {
			expect(mockState.handleApiError).toHaveBeenCalledWith(error);
		});
	});
});
