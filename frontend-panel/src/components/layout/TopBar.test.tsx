import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TopBar } from "@/components/layout/TopBar";

const mockState = vi.hoisted(() => ({
	navigate: vi.fn(),
	fileStore: {
		search: vi.fn(),
		clearSearch: vi.fn(),
		searchQuery: null as string | null,
	},
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => `translated:${key}`,
	}),
}));

vi.mock("react-router-dom", () => ({
	useNavigate: () => mockState.navigate,
	useLocation: () => ({
		pathname: window.location.pathname,
	}),
}));

vi.mock("@/stores/fileStore", () => ({
	useFileStore: (selector: (state: typeof mockState.fileStore) => unknown) =>
		selector(mockState.fileStore),
}));

vi.mock("@/components/layout/HeaderControls", () => ({
	HeaderControls: ({ actions }: { actions?: React.ReactNode }) => (
		<div>
			HeaderControls
			{actions}
		</div>
	),
}));

vi.mock("@/components/layout/TopBarShell", () => ({
	TopBarShell: ({
		left,
		center,
		right,
	}: {
		left: React.ReactNode;
		center?: React.ReactNode;
		right: React.ReactNode;
	}) => (
		<div>
			<div>{left}</div>
			<div>{center}</div>
			<div>{right}</div>
		</div>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => (
		<span data-testid="icon" data-name={name} />
	),
}));

vi.mock("@/components/ui/input", () => ({
	Input: ({
		value,
		onChange,
		onKeyDown,
		placeholder,
		className,
	}: {
		value?: string;
		onChange?: (event: React.ChangeEvent<HTMLInputElement>) => void;
		onKeyDown?: (event: React.KeyboardEvent<HTMLInputElement>) => void;
		placeholder?: string;
		className?: string;
	}) => (
		<input
			value={value}
			onChange={onChange}
			onKeyDown={onKeyDown}
			placeholder={placeholder}
			className={className}
		/>
	),
}));

describe("TopBar", () => {
	beforeEach(() => {
		mockState.navigate.mockReset();
		mockState.fileStore.search.mockReset();
		mockState.fileStore.clearSearch.mockReset();
		mockState.fileStore.searchQuery = null;
		window.history.replaceState({}, "", "/");
	});

	it("renders the logo, search input, and forwarded actions", () => {
		render(
			<TopBar
				onSidebarToggle={vi.fn()}
				actions={<button type="button">Extra</button>}
			/>,
		);

		expect(screen.getByAltText("translated:app_name")).toBeInTheDocument();
		expect(
			screen.getByPlaceholderText("translated:search_placeholder"),
		).toBeInTheDocument();
		expect(screen.getByText("HeaderControls")).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Extra" })).toBeInTheDocument();
		expect(screen.getAllByTestId("icon")[0]).toHaveAttribute(
			"data-name",
			"MagnifyingGlass",
		);
	});

	it("searches trimmed input and navigates to the root route when needed", () => {
		window.history.replaceState({}, "", "/trash");

		render(<TopBar onSidebarToggle={vi.fn()} />);

		const input = screen.getByPlaceholderText(
			"translated:search_placeholder",
		) as HTMLInputElement;
		fireEvent.change(input, { target: { value: "  report  " } });
		fireEvent.keyDown(input, { key: "Enter" });

		expect(mockState.navigate).toHaveBeenCalledWith("/");
		expect(mockState.fileStore.search).toHaveBeenCalledWith("report");
	});

	it("does not search for blank input and skips redundant navigation on root", () => {
		render(<TopBar onSidebarToggle={vi.fn()} />);

		const input = screen.getByPlaceholderText(
			"translated:search_placeholder",
		) as HTMLInputElement;
		fireEvent.change(input, { target: { value: "   " } });
		fireEvent.keyDown(input, { key: "Enter" });

		expect(mockState.navigate).not.toHaveBeenCalled();
		expect(mockState.fileStore.search).not.toHaveBeenCalled();
	});

	it("clears active searches with escape and the clear button", () => {
		mockState.fileStore.searchQuery = "report";

		const { rerender } = render(<TopBar onSidebarToggle={vi.fn()} />);
		const input = screen.getByPlaceholderText(
			"translated:search_placeholder",
		) as HTMLInputElement;

		fireEvent.change(input, { target: { value: "report" } });
		fireEvent.keyDown(input, { key: "Escape" });

		expect(mockState.fileStore.clearSearch).toHaveBeenCalledTimes(1);
		expect(input.value).toBe("");

		fireEvent.change(input, { target: { value: "report" } });
		fireEvent.click(
			screen.getByRole("button", { name: "translated:clear_search" }),
		);

		expect(mockState.fileStore.clearSearch).toHaveBeenCalledTimes(2);
		expect(input.value).toBe("");

		mockState.fileStore.searchQuery = null;
		rerender(<TopBar onSidebarToggle={vi.fn()} />);
		expect(input.value).toBe("");
		expect(
			screen.queryByRole("button", { name: "translated:clear_search" }),
		).not.toBeInTheDocument();
	});
});
