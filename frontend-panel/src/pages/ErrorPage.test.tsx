import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ErrorPage from "@/pages/ErrorPage";

const mockState = vi.hoisted(() => ({
	error: null as unknown,
	navigate: vi.fn(),
}));

const mockRuntime = vi.hoisted(() => ({
	appMode: "development",
	isDev: true,
	isProd: false,
	showDeveloperErrorDetails: true,
}));

const translations = vi.hoisted(
	() =>
		({
			error_page_default_label: "System Fault",
			error_page_default_title:
				"Something broke before this page could finish rendering.",
			error_page_default_description:
				"The interface caught an unexpected problem and stopped before it could show a half-broken view.",
			error_page_default_suggestion:
				"Go back to the previous screen, then retry the last action.",
			error_page_sign_in_required_label: "Sign-In Required",
			error_page_sign_in_required_title:
				"Your session is no longer authorized for this request.",
			error_page_sign_in_required_description:
				"The route is reachable, but the current session cannot open it without a fresh login.",
			error_page_sign_in_required_suggestion:
				"Return to a safe page, sign in again, and retry from there.",
			error_page_access_blocked_label: "Access Blocked",
			error_page_access_blocked_title:
				"This area is outside your current permissions.",
			error_page_access_blocked_description:
				"The app reached the resource, but your account is not allowed to open it.",
			error_page_access_blocked_suggestion:
				"Switch to an account with access or ask an administrator to grant permission.",
			error_page_route_missing_label: "Route Missing",
			error_page_route_missing_title: "This route drifted out of range.",
			error_page_route_missing_description:
				"The address may be outdated, typed incorrectly, or mapped to a page that has already moved.",
			error_page_route_missing_suggestion:
				"Double-check the address or jump back to a known location.",
			error_page_server_fault_label: "Server Fault",
			error_page_server_fault_title: "The page failed while trying to load.",
			error_page_server_fault_description:
				"The request made it through the interface, but something upstream failed before the page could recover.",
			error_page_server_fault_suggestion:
				"Retry the request. If it keeps failing, inspect the backend or server logs.",
			error_page_default_message: "An unexpected error occurred.",
			error_page_missing_route_message:
				"This address does not map to a configured route.",
			error_page_root_workspace: "Root workspace",
			error_page_runtime_exception: "Runtime exception",
			error_page_status_heading: "Status",
			error_page_developer_detail: "Developer detail",
			error_page_error_name: "Error name",
			error_page_message: "Message",
			error_page_payload: "Payload",
			error_page_stack: "Stack",
			error_page_path: "Path",
			error_page_response: "Response",
			error_page_recovery_title: "Recommended recovery path",
			error_page_go_back: "Go Back",
			error_page_go_home: "Go Home",
		}) satisfies Record<string, string>,
);

vi.mock("@/config/runtime", () => ({
	runtimeFlags: mockRuntime,
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, options?: Record<string, unknown>) =>
			key === "error_page_response_http"
				? `HTTP ${String(options?.status ?? "")}`
				: (translations[key] ?? key),
	}),
}));

vi.mock("react-router-dom", () => ({
	isRouteErrorResponse: (value: unknown) =>
		typeof value === "object" &&
		value !== null &&
		"status" in value &&
		"statusText" in value,
	useLocation: () => ({ pathname: "/broken-link" }),
	useNavigate: () => mockState.navigate,
	useRouteError: () => mockState.error,
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
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
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => (
		<span data-testid="icon" data-name={name} />
	),
}));

describe("ErrorPage", () => {
	beforeEach(() => {
		mockState.error = null;
		mockState.navigate.mockReset();
		mockRuntime.showDeveloperErrorDetails = true;
	});

	it("renders route response errors with status and status text", () => {
		mockState.error = { status: 404, statusText: "Not Found" };

		render(<ErrorPage />);

		expect(screen.getByText("404")).toBeInTheDocument();
		expect(screen.getByText("Not Found")).toBeInTheDocument();
		expect(
			screen.getByText("This route drifted out of range."),
		).toBeInTheDocument();
		expect(screen.getAllByTestId("icon")[0]).toHaveAttribute(
			"data-name",
			"CircleAlert",
		);
	});

	it("renders generic Error instances and navigates home", () => {
		mockState.error = new Error("Boom");

		render(<ErrorPage />);

		expect(screen.queryByText("404")).not.toBeInTheDocument();
		expect(screen.getByText("Boom")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: /Go Home/i }));

		expect(mockState.navigate).toHaveBeenCalledWith("/");
	});

	it("hides raw error details in release mode", () => {
		mockState.error = new Error("Boom");
		mockRuntime.showDeveloperErrorDetails = false;

		render(<ErrorPage />);

		expect(screen.queryByText("Boom")).not.toBeInTheDocument();
		expect(screen.queryByText("Developer detail")).not.toBeInTheDocument();
		expect(screen.queryByText("Path")).not.toBeInTheDocument();
		expect(screen.queryByText("Response")).not.toBeInTheDocument();
		expect(
			screen.queryByText("Recommended recovery path"),
		).not.toBeInTheDocument();
	});

	it("treats missing route errors as a 404 state", () => {
		render(<ErrorPage />);

		expect(screen.getByText("404")).toBeInTheDocument();
		expect(
			screen.getByText("This address does not map to a configured route."),
		).toBeInTheDocument();
		expect(screen.getByText("/broken-link")).toBeInTheDocument();
	});

	it("falls back to the default message for unknown errors", () => {
		mockState.error = "unexpected";

		render(<ErrorPage />);

		expect(
			screen.getByText("An unexpected error occurred."),
		).toBeInTheDocument();
	});
});
