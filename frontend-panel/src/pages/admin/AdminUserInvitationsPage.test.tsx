import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AdminUserInvitationsPage from "@/pages/admin/AdminUserInvitationsPage";
import type {
	AdminUserInvitationInfo,
	CreateUserInvitationRequest,
} from "@/types/api";

const mockState = vi.hoisted(() => ({
	createInvitation: vi.fn(),
	handleApiError: vi.fn(),
	listInvitations: vi.fn(),
	revokeInvitation: vi.fn(),
	toastSuccess: vi.fn(),
	writeTextToClipboard: vi.fn(),
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, options?: Record<string, unknown>) => {
			if (key === "entries_page") {
				return `entries:${options?.current}/${options?.pages}/${options?.total}`;
			}
			if (key === "page_size_option") {
				return `page-size:${options?.count}`;
			}
			if (key === "confirm_revoke_invitation") {
				return `confirm_revoke_invitation:${options?.email}`;
			}
			return key.replace(/^core:/, "");
		},
	}),
}));

vi.mock("sonner", () => ({
	toast: {
		success: (...args: unknown[]) => mockState.toastSuccess(...args),
	},
}));

vi.mock("@/components/admin/AdminOffsetPagination", () => ({
	AdminOffsetPagination: ({
		onNext,
		onPageSizeChange,
		onPrevious,
		pageSizeOptions,
		total,
	}: {
		onNext: () => void;
		onPageSizeChange: (value: string | null) => void;
		onPrevious: () => void;
		pageSizeOptions: Array<{ label: string; value: string }>;
		total: number;
	}) =>
		total > 0 ? (
			<div>
				<button type="button" onClick={onPrevious}>
					prev-page
				</button>
				<button type="button" onClick={onNext}>
					next-page
				</button>
				<button
					type="button"
					onClick={() => onPageSizeChange(pageSizeOptions[0]?.value ?? null)}
				>
					page-size
				</button>
			</div>
		) : null,
}));

vi.mock("@/components/admin/admin-users-page/InviteUserDialog", () => ({
	InviteUserDialog: ({
		createdInvitation,
		form,
		onCopyLink,
		onFieldChange,
		onFieldValidate,
		onSubmit,
		open,
	}: {
		createdInvitation: AdminUserInvitationInfo | null;
		form: CreateUserInvitationRequest;
		onCopyLink: (value: string) => void;
		onFieldChange: (
			key: keyof CreateUserInvitationRequest,
			value: string,
		) => void;
		onFieldValidate: (
			key: keyof CreateUserInvitationRequest,
			value: string,
		) => void;
		onSubmit: React.FormEventHandler<HTMLFormElement>;
		open: boolean;
	}) =>
		open ? (
			<form onSubmit={onSubmit}>
				<label htmlFor="invite-email">email</label>
				<input
					id="invite-email"
					value={form.email}
					onChange={(event) => {
						onFieldChange("email", event.target.value);
						onFieldValidate("email", event.target.value.trim());
					}}
				/>
				{createdInvitation?.invitation_url ? (
					<button
						type="button"
						onClick={() => onCopyLink(createdInvitation.invitation_url ?? "")}
					>
						copy-created
					</button>
				) : null}
				<button type="submit">send_invitation</button>
			</form>
		) : null,
}));

vi.mock("@/components/admin/admin-users-page/UserInvitationsTable", () => ({
	UserInvitationsTable: ({
		invitations,
		onCopyLink,
		onRevokeInvitation,
		revokingInvitationId,
	}: {
		invitations: AdminUserInvitationInfo[];
		onCopyLink: (value: string) => void;
		onRevokeInvitation: (invitation: AdminUserInvitationInfo) => void;
		revokingInvitationId: number | null;
	}) => (
		<div>
			{invitations.map((invitation) => (
				<div key={invitation.id}>
					<span>{invitation.email}</span>
					<span>{invitation.status}</span>
					<button
						type="button"
						onClick={() => onCopyLink(invitation.invitation_url ?? "")}
					>
						copy:{invitation.id}
					</button>
					<button
						type="button"
						disabled={revokingInvitationId === invitation.id}
						onClick={() => onRevokeInvitation(invitation)}
					>
						revoke:{invitation.id}
					</button>
				</div>
			))}
		</div>
	),
}));

vi.mock("@/components/common/ConfirmDialog", () => ({
	ConfirmDialog: ({
		confirmLabel,
		description,
		onConfirm,
		open,
		title,
	}: {
		confirmLabel: string;
		description?: string;
		onConfirm: () => void;
		open: boolean;
		title: string;
	}) =>
		open ? (
			<div>
				<div>{title}</div>
				<div>{description}</div>
				<button type="button" onClick={onConfirm}>
					{confirmLabel}
				</button>
			</div>
		) : null,
}));

vi.mock("@/components/common/EmptyState", () => ({
	EmptyState: ({
		action,
		description,
		title,
	}: {
		action?: React.ReactNode;
		description?: string;
		title: string;
	}) => (
		<div>
			<div>{title}</div>
			<div>{description}</div>
			<div>{action}</div>
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
		actions,
		description,
		title,
	}: {
		actions?: React.ReactNode;
		description: string;
		title: string;
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

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		disabled,
		onClick,
		type,
	}: {
		children?: React.ReactNode;
		disabled?: boolean;
		onClick?: () => void;
		type?: "button" | "submit";
	}) => (
		<button type={type ?? "button"} disabled={disabled} onClick={onClick}>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{name}</span>,
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: (...args: unknown[]) => mockState.handleApiError(...args),
}));

vi.mock("@/hooks/usePageTitle", () => ({
	usePageTitle: vi.fn(),
}));

vi.mock("@/lib/clipboard", () => ({
	writeTextToClipboard: (...args: unknown[]) =>
		mockState.writeTextToClipboard(...args),
}));

vi.mock("@/services/adminService", () => ({
	adminUserService: {
		createInvitation: (...args: unknown[]) =>
			mockState.createInvitation(...args),
		listInvitations: (...args: unknown[]) => mockState.listInvitations(...args),
		revokeInvitation: (...args: unknown[]) =>
			mockState.revokeInvitation(...args),
	},
}));

function createInvitation(
	overrides: Partial<AdminUserInvitationInfo> = {},
): AdminUserInvitationInfo {
	return {
		accepted_at: null,
		accepted_user_id: null,
		created_at: "2026-06-07T10:00:00Z",
		email: "invitee@example.com",
		expires_at: "2026-06-10T10:00:00Z",
		id: 101,
		invitation_url: "https://drive.example.test/invite/token",
		invited_by: 1,
		mail_queued: false,
		revoked_at: null,
		status: "pending",
		updated_at: "2026-06-07T10:00:00Z",
		...overrides,
	};
}

function LocationProbe() {
	const location = useLocation();

	return (
		<>
			<div data-testid="location-path">{location.pathname}</div>
			<div data-testid="location-search">{location.search}</div>
		</>
	);
}

function renderPage(initialEntry = "/admin/users/invitations") {
	return render(
		<MemoryRouter initialEntries={[initialEntry]}>
			<LocationProbe />
			<AdminUserInvitationsPage />
		</MemoryRouter>,
	);
}

describe("AdminUserInvitationsPage", () => {
	beforeEach(() => {
		mockState.createInvitation.mockReset();
		mockState.handleApiError.mockReset();
		mockState.listInvitations.mockReset();
		mockState.revokeInvitation.mockReset();
		mockState.toastSuccess.mockReset();
		mockState.writeTextToClipboard.mockReset();

		mockState.createInvitation.mockResolvedValue(createInvitation());
		mockState.listInvitations.mockResolvedValue({
			items: [createInvitation()],
			total: 1,
		});
		mockState.revokeInvitation.mockImplementation(async (id) =>
			createInvitation({
				id,
				revoked_at: "2026-06-07T11:00:00Z",
				status: "revoked",
			}),
		);
		mockState.writeTextToClipboard.mockResolvedValue(undefined);
	});

	it("loads invitations from query params and navigates back to users", async () => {
		renderPage("/admin/users/invitations?offset=20&pageSize=10");

		await waitFor(() => {
			expect(mockState.listInvitations).toHaveBeenCalledWith({
				limit: 10,
				offset: 20,
			});
		});
		expect(screen.getByText("user_invitations")).toBeInTheDocument();
		expect(screen.getByText("invitee@example.com")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: /back_to_users/i }));

		expect(screen.getByTestId("location-path").textContent).toBe(
			"/admin/users",
		);
	});

	it("copies links and revokes pending invitations in place", async () => {
		renderPage();

		expect(await screen.findByText("invitee@example.com")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "copy:101" }));

		await waitFor(() => {
			expect(mockState.writeTextToClipboard).toHaveBeenCalledWith(
				"https://drive.example.test/invite/token",
			);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("copied_to_clipboard");

		fireEvent.click(screen.getByRole("button", { name: "revoke:101" }));

		expect(
			screen.getByText("confirm_revoke_invitation:invitee@example.com"),
		).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "revoke_invitation" }));

		await waitFor(() => {
			expect(mockState.revokeInvitation).toHaveBeenCalledWith(101);
		});
		expect(await screen.findByText("revoked")).toBeInTheDocument();
		expect(mockState.toastSuccess).toHaveBeenCalledWith("invitation_revoked");
	});

	it("creates an invitation and refreshes the current list", async () => {
		mockState.listInvitations
			.mockResolvedValueOnce({
				items: [],
				total: 0,
			})
			.mockResolvedValueOnce({
				items: [createInvitation({ email: "new@example.com" })],
				total: 1,
			});
		mockState.createInvitation.mockResolvedValueOnce(
			createInvitation({ email: "new@example.com" }),
		);

		renderPage();

		expect(await screen.findByText("no_invitations")).toBeInTheDocument();

		fireEvent.click(
			screen.getAllByRole("button", { name: /invite_user/i })[0] as HTMLElement,
		);
		fireEvent.change(screen.getByLabelText("email"), {
			target: { value: " new@example.com " },
		});
		fireEvent.click(screen.getByRole("button", { name: "send_invitation" }));

		await waitFor(() => {
			expect(mockState.createInvitation).toHaveBeenCalledWith({
				email: "new@example.com",
			});
		});
		await waitFor(() => {
			expect(mockState.listInvitations).toHaveBeenCalledTimes(2);
		});
		expect(await screen.findByText("new@example.com")).toBeInTheDocument();
		expect(mockState.toastSuccess).toHaveBeenCalledWith("invitation_created");
	});
});
