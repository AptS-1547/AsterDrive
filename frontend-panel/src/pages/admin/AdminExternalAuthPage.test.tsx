import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AdminExternalAuthPage from "@/pages/admin/AdminExternalAuthPage";

const mockState = vi.hoisted(() => ({
	create: vi.fn(),
	deleteProvider: vi.fn(),
	handleApiError: vi.fn(),
	list: vi.fn(),
	listKinds: vi.fn(),
	test: vi.fn(),
	toastSuccess: vi.fn(),
	update: vi.fn(),
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, options?: Record<string, unknown>) => {
			if (key === "policy_wizard_progress") {
				return `${options?.current}/${options?.total}`;
			}
			return key;
		},
	}),
}));

vi.mock("sonner", () => ({
	toast: {
		error: vi.fn(),
		success: (...args: unknown[]) => mockState.toastSuccess(...args),
	},
}));

vi.mock("@/components/common/ConfirmDialog", () => ({
	ConfirmDialog: () => null,
}));

vi.mock("@/components/common/EmptyState", () => ({
	EmptyState: ({
		description,
		title,
	}: {
		description: string;
		title: string;
	}) => (
		<div>
			<h2>{title}</h2>
			<p>{description}</p>
		</div>
	),
}));

vi.mock("@/components/common/SkeletonTable", () => ({
	SkeletonTable: () => <div data-testid="skeleton-table" />,
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
		<header>
			<h1>{title}</h1>
			<p>{description}</p>
			<div>{actions}</div>
		</header>
	),
}));

vi.mock("@/components/layout/AdminPageShell", () => ({
	AdminPageShell: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/components/layout/AdminSurface", () => ({
	AdminSurface: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => <section className={className}>{children}</section>,
}));

vi.mock("@/components/ui/badge", () => ({
	Badge: ({ children }: { children: React.ReactNode }) => (
		<span>{children}</span>
	),
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		"aria-label": ariaLabel,
		children,
		disabled,
		onClick,
		title,
		type,
	}: {
		"aria-label"?: string;
		children: React.ReactNode;
		disabled?: boolean;
		onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
		title?: string;
		type?: "button" | "submit";
	}) => (
		<button
			type={type ?? "button"}
			aria-label={ariaLabel}
			disabled={disabled}
			onClick={onClick}
			title={title}
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
	DialogDescription: ({ children }: { children: React.ReactNode }) => (
		<p>{children}</p>
	),
	DialogFooter: ({ children }: { children: React.ReactNode }) => (
		<footer>{children}</footer>
	),
	DialogHeader: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DialogTitle: ({ children }: { children: React.ReactNode }) => (
		<h2>{children}</h2>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: () => <span aria-hidden="true" />,
}));

vi.mock("@/components/ui/switch", () => ({
	Switch: ({
		checked,
		id,
		onCheckedChange,
	}: {
		checked: boolean;
		id?: string;
		onCheckedChange: (checked: boolean) => void;
	}) => (
		<input
			id={id}
			type="checkbox"
			checked={checked}
			onChange={(event) => onCheckedChange(event.target.checked)}
		/>
	),
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: (...args: unknown[]) => mockState.handleApiError(...args),
}));

vi.mock("@/hooks/useConfirmDialog", () => ({
	useConfirmDialog: (handler: (id: number) => Promise<void>) => {
		const [confirmId, setConfirmId] = useState<number | null>(null);
		return {
			confirmId,
			dialogProps: {
				onConfirm: () => {
					if (confirmId !== null) {
						void handler(confirmId);
					}
				},
				open: confirmId !== null,
			},
			requestConfirm: (id: number) => setConfirmId(id),
		};
	},
}));

vi.mock("@/hooks/usePageTitle", () => ({
	usePageTitle: vi.fn(),
}));

vi.mock("@/lib/clipboard", () => ({
	writeTextToClipboard: vi.fn(),
}));

vi.mock("@/services/adminService", () => ({
	adminExternalAuthService: {
		create: (...args: unknown[]) => mockState.create(...args),
		delete: (...args: unknown[]) => mockState.deleteProvider(...args),
		list: (...args: unknown[]) => mockState.list(...args),
		listKinds: (...args: unknown[]) => mockState.listKinds(...args),
		test: (...args: unknown[]) => mockState.test(...args),
		update: (...args: unknown[]) => mockState.update(...args),
	},
}));

describe("AdminExternalAuthPage", () => {
	beforeEach(() => {
		mockState.create.mockReset();
		mockState.deleteProvider.mockReset();
		mockState.handleApiError.mockReset();
		mockState.list.mockReset();
		mockState.listKinds.mockReset();
		mockState.test.mockReset();
		mockState.toastSuccess.mockReset();
		mockState.update.mockReset();

		mockState.listKinds.mockResolvedValue([
			{
				default_scopes: "openid email profile",
				description: "OpenID Connect authorization-code sign-in.",
				display_name: "OpenID Connect",
				kind: "oidc",
				protocol: "oidc",
				supports_discovery: true,
				supports_email_verified_claim: true,
				supports_pkce: true,
			},
		]);
		mockState.list.mockResolvedValue([]);
		mockState.create.mockResolvedValue({
			allowed_domains: ["example.com"],
			auto_link_verified_email_enabled: false,
			auto_provision_enabled: false,
			client_id: "client-123",
			client_secret: null,
			client_secret_configured: false,
			created_at: "2026-05-17T10:00:00Z",
			display_name: "Example IDP",
			display_name_claim: null,
			email_claim: null,
			enabled: false,
			groups_claim: null,
			id: 1,
			issuer_url: "https://idp.example.com",
			key: "example",
			protocol: "oidc",
			provider_kind: "oidc",
			require_email_verified: true,
			scopes: "openid email profile",
			updated_at: "2026-05-17T10:00:00Z",
			username_claim: null,
		});
	});

	it("creates a provider from the SSO type wizard with provider_kind", async () => {
		render(<AdminExternalAuthPage />);

		await waitFor(() => expect(mockState.listKinds).toHaveBeenCalled());
		const createButtons = screen.getAllByRole("button", {
			name: /external_auth_provider_create/,
		});
		fireEvent.click(createButtons[createButtons.length - 1]);

		expect(screen.getByText("OpenID Connect")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "policy_wizard_next" }));

		fireEvent.change(screen.getByLabelText("external_auth_provider_key"), {
			target: { value: "Example" },
		});
		fireEvent.change(
			screen.getByLabelText("external_auth_provider_display_name"),
			{
				target: { value: "Example IDP" },
			},
		);
		fireEvent.change(
			screen.getByLabelText("external_auth_provider_issuer_url"),
			{
				target: { value: "https://idp.example.com" },
			},
		);
		fireEvent.change(
			screen.getByLabelText("external_auth_provider_client_id"),
			{
				target: { value: "client-123" },
			},
		);
		fireEvent.click(
			screen.getByRole("button", { name: "policy_wizard_review" }),
		);

		fireEvent.change(
			screen.getByLabelText("external_auth_provider_allowed_domains"),
			{
				target: { value: "Example.COM, example.com" },
			},
		);
		const submitButtons = screen.getAllByRole("button", {
			name: /external_auth_provider_create/,
		});
		fireEvent.click(submitButtons[submitButtons.length - 1]);

		await waitFor(() => expect(mockState.create).toHaveBeenCalledTimes(1));
		expect(mockState.create).toHaveBeenCalledWith(
			expect.objectContaining({
				allowed_domains: ["example.com"],
				client_id: "client-123",
				display_name: "Example IDP",
				issuer_url: "https://idp.example.com",
				key: "Example",
				provider_kind: "oidc",
				scopes: "openid email profile",
			}),
		);
	});
});
