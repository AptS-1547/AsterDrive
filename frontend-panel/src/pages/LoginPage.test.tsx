import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import LoginPage from "@/pages/LoginPage";
import { useThemeStore } from "@/stores/themeStore";

const MockApiError = vi.hoisted(
	() =>
		class MockApiError extends Error {
			code: number;
			constructor(code: number, message: string) {
				super(message);
				this.code = code;
			}
		},
);

const mockState = vi.hoisted(() => ({
	check: vi.fn(),
	handleApiError: vi.fn(),
	login: vi.fn(),
	location: {
		hash: "",
		pathname: "/login",
		search: "",
	},
	navigate: vi.fn(),
	register: vi.fn(),
	resendRegisterActivation: vi.fn(),
	setup: vi.fn(),
	toastError: vi.fn(),
	toastSuccess: vi.fn(),
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
	}),
}));

vi.mock("react-router-dom", () => ({
	useLocation: () => mockState.location,
	useNavigate: () => mockState.navigate,
}));

vi.mock("@/services/http", () => ({
	ApiError: MockApiError,
}));

vi.mock("@/types/api-helpers", () => ({
	ErrorCode: {
		PendingActivation: 2004,
	},
}));

vi.mock("sonner", () => ({
	toast: {
		error: (...args: unknown[]) => mockState.toastError(...args),
		success: (...args: unknown[]) => mockState.toastSuccess(...args),
	},
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		type,
		disabled,
		onClick,
		className,
	}: {
		children: React.ReactNode;
		type?: "button" | "submit";
		disabled?: boolean;
		onClick?: () => void;
		className?: string;
	}) => (
		<button
			type={type ?? "button"}
			disabled={disabled}
			onClick={onClick}
			className={className}
		>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{name}</span>,
}));

vi.mock("@/components/ui/input", () => ({
	Input: ({ ...props }: React.InputHTMLAttributes<HTMLInputElement>) => (
		<input {...props} />
	),
}));

vi.mock("@/components/ui/label", () => ({
	Label: ({
		children,
		htmlFor,
		className,
	}: {
		children: React.ReactNode;
		htmlFor?: string;
		className?: string;
	}) => (
		<label htmlFor={htmlFor} className={className}>
			{children}
		</label>
	),
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: (...args: unknown[]) => mockState.handleApiError(...args),
}));

vi.mock("@/lib/validation", () => ({
	emailSchema: {
		safeParse: (value: string) =>
			/^[^@]+@[^@]+\.[^@]+$/.test(value)
				? { success: true }
				: {
						error: { issues: [{ message: "invalid-email" }] },
						success: false,
					},
	},
	passwordSchema: {
		safeParse: (value: string) =>
			value.length >= 6
				? { success: true }
				: {
						error: { issues: [{ message: "invalid-password" }] },
						success: false,
					},
	},
	usernameSchema: {
		safeParse: (value: string) =>
			/^[a-zA-Z0-9_-]{4,16}$/.test(value)
				? { success: true }
				: {
						error: { issues: [{ message: "invalid-username" }] },
						success: false,
					},
	},
}));

vi.mock("@/services/authService", () => ({
	authService: {
		check: (...args: unknown[]) => mockState.check(...args),
		register: (...args: unknown[]) => mockState.register(...args),
		resendRegisterActivation: (...args: unknown[]) =>
			mockState.resendRegisterActivation(...args),
		setup: (...args: unknown[]) => mockState.setup(...args),
	},
}));

vi.mock("@/stores/authStore", () => ({
	useAuthStore: (
		selector: (state: { login: typeof mockState.login }) => unknown,
	) => selector({ login: mockState.login }),
}));

describe("LoginPage", () => {
	beforeEach(() => {
		document.documentElement.classList.remove("dark");
		mockState.check.mockReset();
		mockState.handleApiError.mockReset();
		mockState.login.mockReset();
		mockState.location = {
			hash: "",
			pathname: "/login",
			search: "",
		};
		mockState.navigate.mockReset();
		mockState.register.mockReset();
		mockState.resendRegisterActivation.mockReset();
		mockState.setup.mockReset();
		mockState.toastError.mockReset();
		mockState.toastSuccess.mockReset();
		mockState.login.mockResolvedValue(undefined);
		mockState.register.mockResolvedValue(undefined);
		mockState.resendRegisterActivation.mockResolvedValue(undefined);
		mockState.setup.mockResolvedValue(undefined);
		mockState.check.mockResolvedValue({ exists: true, has_users: true });
		useThemeStore.setState({ resolvedTheme: "light" });
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it("auto-detects login mode and signs existing users in", async () => {
		render(<LoginPage />);

		fireEvent.change(screen.getByLabelText("email_or_username"), {
			target: { value: "user@example.com" },
		});

		await waitFor(() => {
			expect(mockState.check).toHaveBeenCalledWith("user@example.com");
		});
		expect(
			await screen.findByRole("button", { name: "sign_in" }),
		).toBeInTheDocument();

		fireEvent.change(screen.getByLabelText("password"), {
			target: { value: "secret123" },
		});
		fireEvent.click(screen.getByRole("button", { name: "sign_in" }));

		await waitFor(() => {
			expect(mockState.login).toHaveBeenCalledWith(
				"user@example.com",
				"secret123",
			);
		});
		await waitFor(() => {
			expect(mockState.navigate).toHaveBeenCalledWith("/", { replace: true });
		});
	});

	it("shows a query toast passed from the verification redirect", async () => {
		mockState.location = {
			hash: "",
			pathname: "/login",
			search: "?contact_verification=email-changed&email=changed%40example.com",
		};

		render(<LoginPage />);

		await waitFor(() =>
			expect(mockState.toastSuccess).toHaveBeenCalledWith(
				"settings:settings_email_change_confirmed_login_hint",
				{
					id: "contact-verification-email-changed-login:changed@example.com",
				},
			),
		);
		expect(mockState.navigate).toHaveBeenCalledWith(
			{
				hash: "",
				pathname: "/login",
				search: "",
			},
			{ replace: true },
		);
	});

	it("shows an error toast for expired verification redirects", async () => {
		mockState.location = {
			hash: "",
			pathname: "/login",
			search: "?contact_verification=expired",
		};

		render(<LoginPage />);

		await waitFor(() =>
			expect(mockState.toastError).toHaveBeenCalledWith(
				"verify_contact_expired_title",
				{
					description: "verify_contact_expired_desc",
					id: "contact-verification-expired-login",
				},
			),
		);
		expect(mockState.navigate).toHaveBeenCalledWith(
			{
				hash: "",
				pathname: "/login",
				search: "",
			},
			{ replace: true },
		);
	});

	it("keeps the desktop brand logo matched to the dark hero surface", () => {
		render(<LoginPage />);

		const [desktopLogo, mobileLogo] = screen.getAllByRole("img", {
			name: "AsterDrive",
		});

		expect(desktopLogo).toHaveAttribute(
			"src",
			"/static/asterdrive/asterdrive-light.svg",
		);
		expect(mobileLogo).toHaveAttribute(
			"src",
			"/static/asterdrive/asterdrive-dark.svg",
		);
	});

	it("runs initial setup for the first user and then signs them in", async () => {
		mockState.check.mockResolvedValueOnce({ exists: false, has_users: false });

		render(<LoginPage />);

		fireEvent.change(screen.getByLabelText("email_or_username"), {
			target: { value: "admin@example.com" },
		});

		expect(await screen.findByLabelText("username")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "create_admin" }),
		).toBeInTheDocument();

		fireEvent.change(screen.getByLabelText("username"), {
			target: { value: "adminuser" },
		});
		fireEvent.change(screen.getByLabelText("password"), {
			target: { value: "secret123" },
		});
		fireEvent.click(screen.getByRole("button", { name: "create_admin" }));

		await waitFor(() => {
			expect(mockState.setup).toHaveBeenCalledWith(
				"adminuser",
				"admin@example.com",
				"secret123",
			);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("setup_complete");
		expect(mockState.login).toHaveBeenCalledWith(
			"admin@example.com",
			"secret123",
		);
		await waitFor(() => {
			expect(mockState.navigate).toHaveBeenCalledWith("/", { replace: true });
		});
	});

	it("shows an activation waiting state after register instead of logging in", async () => {
		mockState.check.mockResolvedValueOnce({ exists: false, has_users: true });

		render(<LoginPage />);

		fireEvent.change(screen.getByLabelText("email_or_username"), {
			target: { value: "new@example.com" },
		});

		fireEvent.change(await screen.findByLabelText("username"), {
			target: { value: "newuser" },
		});
		fireEvent.change(screen.getByLabelText("password"), {
			target: { value: "secret123" },
		});
		fireEvent.click(screen.getByRole("button", { name: "sign_up" }));

		await waitFor(() => {
			expect(mockState.register).toHaveBeenCalledWith(
				"newuser",
				"new@example.com",
				"secret123",
			);
		});
		expect(mockState.login).not.toHaveBeenCalled();
		expect(
			await screen.findByText("activation_pending_notice"),
		).toBeInTheDocument();

		fireEvent.click(
			await screen.findByRole("button", { name: /resend_activation/ }),
		);

		await waitFor(() => {
			expect(mockState.resendRegisterActivation).toHaveBeenCalledWith(
				"new@example.com",
			);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("register_success");
		expect(mockState.toastSuccess).toHaveBeenCalledWith("activation_resent");
	});

	it("switches pending-activation login failures into the activation state", async () => {
		mockState.login.mockRejectedValueOnce(new MockApiError(2004, "pending"));

		render(<LoginPage />);

		fireEvent.change(screen.getByLabelText("email_or_username"), {
			target: { value: "user@example.com" },
		});
		fireEvent.change(screen.getByLabelText("password"), {
			target: { value: "secret123" },
		});

		await screen.findByRole("button", { name: "sign_in" });
		fireEvent.click(screen.getByRole("button", { name: "sign_in" }));

		expect(
			await screen.findByText("activation_pending_notice"),
		).toBeInTheDocument();

		fireEvent.click(
			await screen.findByRole("button", { name: /resend_activation/ }),
		);

		await waitFor(() => {
			expect(mockState.resendRegisterActivation).toHaveBeenCalledWith(
				"user@example.com",
			);
		});
	});

	it("shows validation errors and reports submit failures without navigating", async () => {
		mockState.check.mockResolvedValueOnce({ exists: true, has_users: true });
		const error = new Error("login failed");
		mockState.login.mockRejectedValueOnce(error);

		render(<LoginPage />);

		fireEvent.change(screen.getByLabelText("email_or_username"), {
			target: { value: "bad" },
		});
		fireEvent.change(screen.getByLabelText("password"), {
			target: { value: "123" },
		});
		fireEvent.click(screen.getByRole("button", { name: "continue" }));

		expect(screen.getByText("invalid-username")).toBeInTheDocument();
		expect(screen.getByText("invalid-password")).toBeInTheDocument();
		expect(mockState.check).not.toHaveBeenCalled();

		fireEvent.change(screen.getByLabelText("email_or_username"), {
			target: { value: "user@example.com" },
		});
		fireEvent.change(screen.getByLabelText("password"), {
			target: { value: "secret123" },
		});

		await waitFor(() => {
			expect(mockState.check).toHaveBeenCalledWith("user@example.com");
		});
		await screen.findByRole("button", { name: "sign_in" });

		fireEvent.click(screen.getByRole("button", { name: "sign_in" }));

		await waitFor(() => {
			expect(mockState.handleApiError).toHaveBeenCalledWith(error);
		});
		expect(mockState.navigate).not.toHaveBeenCalled();
	});

	it("keeps submit disabled until login fields are filled", async () => {
		render(<LoginPage />);

		const submitButton = screen.getByRole("button", { name: "continue" });
		expect(submitButton).toBeDisabled();

		fireEvent.change(screen.getByLabelText("email_or_username"), {
			target: { value: "user@example.com" },
		});
		expect(submitButton).toBeDisabled();

		fireEvent.change(screen.getByLabelText("password"), {
			target: { value: "secret123" },
		});

		await waitFor(() => {
			expect(screen.getByRole("button", { name: "sign_in" })).toBeEnabled();
		});
	});

	it("keeps submit disabled until register fields are filled", async () => {
		mockState.check.mockResolvedValueOnce({ exists: false, has_users: true });

		render(<LoginPage />);

		fireEvent.change(screen.getByLabelText("email_or_username"), {
			target: { value: "new@example.com" },
		});
		fireEvent.change(screen.getByLabelText("password"), {
			target: { value: "secret123" },
		});

		await waitFor(() => {
			expect(screen.getByRole("button", { name: "sign_up" })).toBeDisabled();
		});

		fireEvent.change(await screen.findByLabelText("username"), {
			target: { value: "newuser" },
		});

		await waitFor(() => {
			expect(screen.getByRole("button", { name: "sign_up" })).toBeEnabled();
		});
	});

	it("retries auth checks for the same identifier after a failed precheck", async () => {
		vi.useFakeTimers();
		mockState.check
			.mockRejectedValueOnce(new Error("network error"))
			.mockResolvedValueOnce({ exists: true, has_users: true });

		render(<LoginPage />);

		const identifierInput = screen.getByLabelText("email_or_username");

		fireEvent.change(identifierInput, {
			target: { value: "user@example.com" },
		});
		await vi.advanceTimersByTimeAsync(500);
		expect(mockState.check).toHaveBeenCalledTimes(1);
		expect(mockState.check).toHaveBeenNthCalledWith(1, "user@example.com");

		fireEvent.change(identifierInput, {
			target: { value: "user@example.co" },
		});
		fireEvent.change(identifierInput, {
			target: { value: "user@example.com" },
		});
		await vi.advanceTimersByTimeAsync(500);

		expect(mockState.check).toHaveBeenCalledTimes(2);
		expect(mockState.check).toHaveBeenNthCalledWith(2, "user@example.com");
	});
});
