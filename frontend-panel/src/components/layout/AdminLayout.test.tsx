import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { setPublicSiteUrl } from "@/lib/publicSiteUrl";

const mockState = vi.hoisted(() => ({
	brandingLoaded: false,
	currentPath: "/admin/users",
	handleApiError: vi.fn(),
	setConfig: vi.fn(),
	siteUrl: null as string | null,
	toastSuccess: vi.fn(),
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => `translated:${key}`,
	}),
}));

vi.mock("sonner", () => ({
	toast: {
		success: (...args: unknown[]) => mockState.toastSuccess(...args),
	},
}));

vi.mock("react-router-dom", () => ({
	NavLink: ({
		to,
		onClick,
		className,
		children,
	}: {
		to: string;
		onClick?: () => void;
		className?: string | ((state: { isActive: boolean }) => string);
		children: React.ReactNode;
	}) => (
		<button
			type="button"
			onClick={onClick}
			className={
				typeof className === "function"
					? className({ isActive: to === mockState.currentPath })
					: className
			}
		>
			{children}
		</button>
	),
}));

vi.mock("@/components/common/ConfirmDialog", () => ({
	ConfirmDialog: ({
		confirmLabel,
		description,
		onConfirm,
		onOpenChange,
		open,
		title,
	}: {
		confirmLabel?: string;
		description?: string;
		onConfirm: () => void;
		onOpenChange: (open: boolean) => void;
		open: boolean;
		title: string;
	}) =>
		open ? (
			<div>
				<h2>{title}</h2>
				{description ? <p>{description}</p> : null}
				<button type="button" onClick={() => onOpenChange(false)}>
					cancel
				</button>
				<button type="button" onClick={onConfirm}>
					{confirmLabel ?? "confirm"}
				</button>
			</div>
		) : null,
}));

vi.mock("@/components/layout/AdminTopBar", () => ({
	AdminTopBar: ({ onSidebarToggle }: { onSidebarToggle: () => void }) => (
		<button type="button" onClick={onSidebarToggle}>
			Toggle Admin Sidebar
		</button>
	),
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: (...args: unknown[]) => mockState.handleApiError(...args),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => (
		<span data-testid="icon" data-name={name} />
	),
}));

vi.mock("@/components/ui/scroll-area", () => ({
	ScrollArea: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/services/adminService", () => ({
	adminConfigService: {
		set: (...args: unknown[]) => mockState.setConfig(...args),
	},
}));

vi.mock("@/stores/brandingStore", () => {
	const useBrandingStore = ((
		selector: (state: { isLoaded: boolean; siteUrl: string | null }) => unknown,
	) =>
		selector({
			isLoaded: mockState.brandingLoaded,
			siteUrl: mockState.siteUrl,
		})) as unknown as typeof import("@/stores/brandingStore").useBrandingStore;

	useBrandingStore.setState = (partial: { siteUrl?: string | null }) => {
		if ("siteUrl" in partial) {
			mockState.siteUrl = partial.siteUrl ?? null;
		}
	};

	return { useBrandingStore };
});

describe("AdminLayout", () => {
	beforeEach(() => {
		mockState.brandingLoaded = false;
		mockState.currentPath = "/admin/users";
		mockState.handleApiError.mockReset();
		mockState.setConfig.mockReset();
		mockState.siteUrl = null;
		mockState.toastSuccess.mockReset();
		mockState.setConfig.mockResolvedValue({
			key: "public_site_url",
			value: window.location.origin,
		});
		setPublicSiteUrl(null);
	});

	it("renders the translated navigation and main content", () => {
		render(<AdminLayout>Admin Content</AdminLayout>);

		expect(screen.getByText("Admin Content")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /translated:overview/i }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /translated:users/i }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /translated:policies/i }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /translated:policy_groups/i }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /translated:shares/i }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /translated:audit_log/i }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /translated:about/i }),
		).toBeInTheDocument();
		expect(screen.getAllByTestId("icon")).toHaveLength(10);
	});

	it("opens the mobile sidebar overlay and closes it again", () => {
		render(<AdminLayout>Admin Content</AdminLayout>);

		expect(
			screen.queryByRole("button", {
				name: "translated:core:close_admin_sidebar",
			}),
		).not.toBeInTheDocument();

		fireEvent.click(
			screen.getByRole("button", { name: "Toggle Admin Sidebar" }),
		);
		expect(
			screen.getByRole("button", {
				name: "translated:core:close_admin_sidebar",
			}),
		).toBeInTheDocument();

		fireEvent.click(
			screen.getByRole("button", {
				name: "translated:core:close_admin_sidebar",
			}),
		);
		expect(
			screen.queryByRole("button", {
				name: "translated:core:close_admin_sidebar",
			}),
		).not.toBeInTheDocument();
	});

	it("closes the mobile sidebar when a nav link is selected", () => {
		render(<AdminLayout>Admin Content</AdminLayout>);

		fireEvent.click(
			screen.getByRole("button", { name: "Toggle Admin Sidebar" }),
		);
		fireEvent.click(screen.getByRole("button", { name: /translated:locks/i }));

		expect(
			screen.queryByRole("button", {
				name: "translated:core:close_admin_sidebar",
			}),
		).not.toBeInTheDocument();
	});

	it("shows the site URL mismatch prompt on each admin page visit and can update the config", async () => {
		mockState.brandingLoaded = true;
		mockState.siteUrl = "https://configured.example.com";

		const { unmount } = render(<AdminLayout>Admin Content</AdminLayout>);

		expect(
			screen.getByText("translated:site_url_mismatch_title"),
		).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "cancel" }));
		expect(
			screen.queryByText("translated:site_url_mismatch_title"),
		).not.toBeInTheDocument();

		unmount();
		render(<AdminLayout>Admin Content</AdminLayout>);

		expect(
			screen.getByText("translated:site_url_mismatch_title"),
		).toBeInTheDocument();

		fireEvent.click(
			screen.getByRole("button", {
				name: "translated:site_url_mismatch_confirm",
			}),
		);

		await waitFor(() => {
			expect(mockState.setConfig).toHaveBeenCalledWith(
				"public_site_url",
				window.location.origin,
			);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith(
			"translated:settings_saved",
		);
	});
});
