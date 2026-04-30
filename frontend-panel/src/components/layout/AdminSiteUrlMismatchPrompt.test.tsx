import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AdminSiteUrlMismatchPrompt } from "@/components/layout/AdminSiteUrlMismatchPrompt";
import { setPublicSiteUrls } from "@/lib/publicSiteUrl";

const mockState = vi.hoisted(() => ({
	brandingLoaded: false,
	handleApiError: vi.fn(),
	setConfig: vi.fn(),
	siteUrl: null as string | null,
	toastSuccess: vi.fn(),
}));

function AdminRouteShell({ page }: { page: string }) {
	return (
		<div>
			<AdminSiteUrlMismatchPrompt />
			<div>{page}</div>
		</div>
	);
}

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

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: (...args: unknown[]) => mockState.handleApiError(...args),
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

describe("AdminSiteUrlMismatchPrompt", () => {
	beforeEach(() => {
		mockState.brandingLoaded = false;
		mockState.handleApiError.mockReset();
		mockState.setConfig.mockReset();
		mockState.siteUrl = null;
		mockState.toastSuccess.mockReset();
		mockState.setConfig.mockResolvedValue({
			key: "public_site_url",
			value: [window.location.origin],
		});
		setPublicSiteUrls(null);
	});

	it("does not reopen while the admin route shell stays mounted", () => {
		mockState.brandingLoaded = true;
		mockState.siteUrl = "https://configured.example.com";

		const { rerender } = render(<AdminRouteShell page="Users" />);

		expect(
			screen.getByText("translated:site_url_mismatch_title"),
		).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "cancel" }));
		expect(
			screen.queryByText("translated:site_url_mismatch_title"),
		).not.toBeInTheDocument();

		rerender(<AdminRouteShell page="Settings" />);

		expect(screen.getByText("Settings")).toBeInTheDocument();
		expect(
			screen.queryByText("translated:site_url_mismatch_title"),
		).not.toBeInTheDocument();
	});

	it("shows the prompt again after leaving admin and can update the config", async () => {
		mockState.brandingLoaded = true;
		mockState.siteUrl = "https://configured.example.com";

		const { unmount } = render(<AdminSiteUrlMismatchPrompt />);

		expect(
			screen.getByText("translated:site_url_mismatch_title"),
		).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "cancel" }));
		expect(
			screen.queryByText("translated:site_url_mismatch_title"),
		).not.toBeInTheDocument();

		unmount();
		render(<AdminSiteUrlMismatchPrompt />);

		expect(
			screen.getByText("translated:site_url_mismatch_title"),
		).toBeInTheDocument();

		fireEvent.click(
			screen.getByRole("button", {
				name: "translated:site_url_mismatch_confirm",
			}),
		);

		await waitFor(() => {
			expect(mockState.setConfig).toHaveBeenCalledWith("public_site_url", [
				window.location.origin,
			]);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith(
			"translated:settings_saved",
		);
	});
});
