import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { HeaderControls } from "@/components/layout/HeaderControls";

const mockState = vi.hoisted(() => ({
	navigate: vi.fn(),
	changeLanguage: vi.fn(),
	queuePreferenceSync: vi.fn(),
	auth: {
		user: {
			email: "alice@example.com",
			profile: {
				display_name: null,
				avatar: {
					source: "none",
					url_512: null,
					url_1024: null,
					version: 0,
				},
			},
			role: "admin",
			username: "alice",
		},
		isAuthStale: false,
		logout: vi.fn(),
	},
	theme: {
		mode: "dark" as "light" | "dark" | "system",
		setMode: vi.fn(),
	},
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		i18n: {
			changeLanguage: mockState.changeLanguage,
			language: "en",
		},
		t: (key: string) => `translated:${key}`,
	}),
}));

vi.mock("react-router-dom", () => ({
	useNavigate: () => mockState.navigate,
}));

vi.mock("@/stores/authStore", () => ({
	useAuthStore: (selector: (state: typeof mockState.auth) => unknown) =>
		selector(mockState.auth),
}));

vi.mock("@/stores/themeStore", () => ({
	useThemeStore: (selector: (state: typeof mockState.theme) => unknown) =>
		selector(mockState.theme),
}));

vi.mock("@/lib/preferenceSync", () => ({
	queuePreferenceSync: (...args: unknown[]) =>
		mockState.queuePreferenceSync(...args),
}));

vi.mock("@/components/common/UserAvatarImage", () => ({
	UserAvatarImage: ({ name }: { name: string }) => (
		<div>{`avatar:${name}`}</div>
	),
}));

vi.mock("@/components/ui/badge", () => ({
	Badge: ({
		children,
		className,
	}: {
		children?: React.ReactNode;
		className?: string;
	}) => <span className={className}>{children}</span>,
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		"aria-label": ariaLabel,
		children,
		className,
		onClick,
		title,
	}: {
		"aria-label"?: string;
		children?: React.ReactNode;
		className?: string;
		onClick?: () => void;
		title?: string;
	}) => (
		<button
			type="button"
			aria-label={ariaLabel}
			className={className}
			onClick={onClick}
			title={title}
		>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => (
		<span data-testid="icon" data-name={name} />
	),
}));

vi.mock("@/components/ui/dropdown-menu", () => ({
	DropdownMenu: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DropdownMenuTrigger: ({
		render,
		children,
	}: {
		render: React.ReactNode;
		children: React.ReactNode;
	}) => (
		<div>
			{render}
			{children}
		</div>
	),
	DropdownMenuContent: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DropdownMenuGroup: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DropdownMenuLabel: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DropdownMenuItem: ({
		children,
		onClick,
	}: {
		children: React.ReactNode;
		onClick?: () => void;
	}) => (
		<button type="button" onClick={onClick}>
			{children}
		</button>
	),
	DropdownMenuSeparator: () => <hr />,
}));

vi.mock("@/components/ui/tooltip", () => ({
	Tooltip: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	TooltipTrigger: ({
		render,
		children,
	}: {
		render: React.ReactNode;
		children: React.ReactNode;
	}) => (
		<div>
			{render}
			{children}
		</div>
	),
	TooltipContent: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

function deferredPromise() {
	let resolve!: () => void;
	const promise = new Promise<void>((nextResolve) => {
		resolve = nextResolve;
	});
	return { promise, resolve };
}

describe("HeaderControls", () => {
	beforeEach(() => {
		mockState.navigate.mockReset();
		mockState.changeLanguage.mockReset();
		mockState.changeLanguage.mockResolvedValue(undefined);
		mockState.queuePreferenceSync.mockReset();
		mockState.auth.user = {
			email: "alice@example.com",
			profile: {
				display_name: null,
				avatar: {
					source: "none",
					url_512: null,
					url_1024: null,
					version: 0,
				},
			},
			role: "admin",
			username: "alice",
		};
		mockState.auth.isAuthStale = false;
		mockState.auth.logout.mockReset();
		mockState.auth.logout.mockResolvedValue(undefined);
		mockState.theme.mode = "dark";
		mockState.theme.setMode.mockReset();
	});

	it("renders actions, stale auth messaging, and admin controls", () => {
		mockState.auth.isAuthStale = true;

		render(
			<HeaderControls
				actions={<button type="button">Refresh</button>}
				showHomeButton
				homeLabel="Home"
				showAdminEntry
			/>,
		);

		expect(screen.getByRole("button", { name: "Refresh" })).toBeInTheDocument();
		expect(
			screen.getByText("translated:offline:offline_status_short"),
		).toBeInTheDocument();
		expect(
			screen.getByText("translated:offline:offline_mode"),
		).toBeInTheDocument();
		expect(
			screen.getByText("translated:offline:auth_stale_detail"),
		).toBeInTheDocument();
		expect(screen.getAllByText("alice")).toHaveLength(3);
		expect(screen.getByText("alice@example.com")).toBeInTheDocument();
		expect(screen.getAllByText("avatar:alice")).toHaveLength(2);
		expect(screen.getAllByText("admin")).toHaveLength(2);
		expect(
			screen.getByRole("button", { name: /translated:settings/i }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /translated:admin_panel/i }),
		).toBeInTheDocument();
		expect(screen.getByText("translated:theme")).toBeInTheDocument();
		expect(screen.getByText("translated:language")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /translated:theme_dark/i }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /translated:language_en/i }),
		).toBeInTheDocument();
		expect(screen.getAllByRole("button", { name: /Home/i })).toHaveLength(2);
	});

	it("uses a stable class set for the account menu trigger", () => {
		render(<HeaderControls showAdminEntry />);

		const trigger = screen.getByRole("button", { name: "alice" });
		expect(trigger.className).toContain("active:translate-y-0");
		expect(trigger.className).toContain(
			"transition-[background-color,border-color,color,box-shadow]",
		);
	});

	it("prefers display_name when rendering the account trigger and menu card", () => {
		mockState.auth.user.profile.display_name = "Alice Chen";

		render(<HeaderControls />);

		expect(screen.getAllByText("Alice Chen")).toHaveLength(3);
		expect(screen.getByText("@alice")).toBeInTheDocument();
		expect(screen.getAllByText("avatar:Alice Chen")).toHaveLength(2);
	});

	it("hides the admin menu entry for non-admin users", () => {
		mockState.auth.user = {
			email: "bob@example.com",
			profile: {
				display_name: null,
				avatar: {
					source: "none",
					url_512: null,
					url_1024: null,
					version: 0,
				},
			},
			role: "user",
			username: "bob",
		};

		render(<HeaderControls showAdminEntry />);

		expect(screen.getAllByText("bob")).toHaveLength(3);
		expect(screen.getAllByText("avatar:bob")).toHaveLength(2);
		expect(
			screen.getByRole("button", { name: /translated:settings/i }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /translated:theme_dark/i }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: /translated:admin_panel/i }),
		).not.toBeInTheDocument();
		expect(screen.queryByText("admin")).not.toBeInTheDocument();
	});

	it("navigates immediately from route entries with view transitions enabled", () => {
		render(<HeaderControls showHomeButton homeLabel="Home" showAdminEntry />);

		fireEvent.click(
			screen.getByRole("button", { name: /translated:settings/i }),
		);
		expect(mockState.navigate).toHaveBeenNthCalledWith(1, "/settings", {
			viewTransition: true,
		});

		const homeButtons = screen.getAllByRole("button", { name: /Home/i });
		expect(homeButtons).toHaveLength(2);

		fireEvent.click(homeButtons[0]);
		expect(mockState.navigate).toHaveBeenNthCalledWith(2, "/", {
			viewTransition: true,
		});

		fireEvent.click(homeButtons[1]);
		expect(mockState.navigate).toHaveBeenNthCalledWith(3, "/", {
			viewTransition: true,
		});

		fireEvent.click(
			screen.getByRole("button", { name: /translated:admin_panel/i }),
		);
		expect(mockState.navigate).toHaveBeenNthCalledWith(4, "/admin", {
			viewTransition: true,
		});
	});

	it("updates theme and language from the account menu", () => {
		render(<HeaderControls />);

		fireEvent.click(
			screen.getByRole("button", { name: /translated:theme_light/i }),
		);
		fireEvent.click(
			screen.getByRole("button", { name: /translated:language_zh/i }),
		);

		expect(mockState.theme.setMode).toHaveBeenCalledWith("light");
		expect(mockState.changeLanguage).toHaveBeenCalledWith("zh");
		expect(mockState.queuePreferenceSync).toHaveBeenCalledWith({
			language: "zh",
		});
	});

	it("shows loading during logout and redirects to login on success", async () => {
		const deferred = deferredPromise();
		mockState.auth.logout.mockReturnValueOnce(deferred.promise);

		render(<HeaderControls />);

		fireEvent.click(screen.getByRole("button", { name: /translated:logout/i }));

		await waitFor(() => {
			expect(
				screen.getByRole("button", { name: /translated:loading/i }),
			).toBeInTheDocument();
		});

		await act(async () => {
			deferred.resolve();
			await deferred.promise;
		});

		expect(mockState.auth.logout).toHaveBeenCalledTimes(1);
		expect(mockState.navigate).toHaveBeenCalledWith("/login", {
			replace: true,
		});
		await waitFor(() => {
			expect(
				screen.getByRole("button", { name: /translated:logout/i }),
			).toBeInTheDocument();
		});
	});
});
