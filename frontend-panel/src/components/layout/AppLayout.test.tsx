import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppLayout } from "@/components/layout/AppLayout";

const mockState = vi.hoisted(() => ({
	shouldIgnoreKeyboardTarget: vi.fn(() => false),
}));

vi.mock("@/hooks/useSelectionShortcuts", () => ({
	shouldIgnoreKeyboardTarget: mockState.shouldIgnoreKeyboardTarget,
}));

vi.mock("@/components/layout/TopBar", () => ({
	TopBar: ({
		onSidebarToggle,
		mobileOpen,
		actions,
		onSearchOpen,
	}: {
		onSidebarToggle: () => void;
		mobileOpen: boolean;
		actions?: React.ReactNode;
		onSearchOpen: () => void;
	}) => (
		<div data-testid="topbar" data-mobile-open={String(mobileOpen)}>
			<button type="button" onClick={onSidebarToggle}>
				Toggle Sidebar
			</button>
			<button type="button" onClick={onSearchOpen}>
				Open Search
			</button>
			{actions}
		</div>
	),
}));

vi.mock("@/components/layout/Sidebar", () => ({
	Sidebar: ({
		mobileOpen,
		onMobileClose,
		onTrashDrop,
		onMoveToFolder,
	}: {
		mobileOpen: boolean;
		onMobileClose: () => void;
		onTrashDrop?: unknown;
		onMoveToFolder?: unknown;
	}) => (
		<div
			data-testid="sidebar"
			data-mobile-open={String(mobileOpen)}
			data-has-trash-drop={String(Boolean(onTrashDrop))}
			data-has-move={String(Boolean(onMoveToFolder))}
		>
			<button type="button" onClick={onMobileClose}>
				Close Sidebar
			</button>
		</div>
	),
}));

vi.mock("@/components/layout/GlobalSearchDialog", () => ({
	GlobalSearchDialog: ({
		open,
	}: {
		open: boolean;
		onOpenChange: (open: boolean) => void;
	}) => <div data-testid="global-search-dialog" data-open={String(open)} />,
}));

describe("AppLayout", () => {
	beforeEach(() => {
		mockState.shouldIgnoreKeyboardTarget.mockReset();
		mockState.shouldIgnoreKeyboardTarget.mockReturnValue(false);
	});

	it("renders children and forwards actions and drag handlers", () => {
		const onTrashDrop = vi.fn();
		const onMoveToFolder = vi.fn();

		render(
			<AppLayout
				actions={<button type="button">Extra</button>}
				onTrashDrop={onTrashDrop}
				onMoveToFolder={onMoveToFolder}
			>
				<div>Page Content</div>
			</AppLayout>,
		);

		expect(screen.getByRole("button", { name: "Extra" })).toBeInTheDocument();
		expect(screen.getByText("Page Content")).toBeInTheDocument();
		expect(screen.getByTestId("topbar")).toHaveAttribute(
			"data-mobile-open",
			"false",
		);
		expect(screen.getByTestId("sidebar")).toHaveAttribute(
			"data-has-trash-drop",
			"true",
		);
		expect(screen.getByTestId("sidebar")).toHaveAttribute(
			"data-has-move",
			"true",
		);
		expect(screen.getByTestId("global-search-dialog")).toHaveAttribute(
			"data-open",
			"false",
		);
	});

	it("toggles and closes the mobile sidebar", () => {
		render(<AppLayout>Page Content</AppLayout>);

		expect(screen.getByTestId("sidebar")).toHaveAttribute(
			"data-mobile-open",
			"false",
		);
		expect(screen.getByTestId("topbar")).toHaveAttribute(
			"data-mobile-open",
			"false",
		);

		fireEvent.click(screen.getByRole("button", { name: "Toggle Sidebar" }));
		expect(screen.getByTestId("sidebar")).toHaveAttribute(
			"data-mobile-open",
			"true",
		);
		expect(screen.getByTestId("topbar")).toHaveAttribute(
			"data-mobile-open",
			"true",
		);

		fireEvent.click(screen.getByRole("button", { name: "Close Sidebar" }));
		expect(screen.getByTestId("sidebar")).toHaveAttribute(
			"data-mobile-open",
			"false",
		);
		expect(screen.getByTestId("topbar")).toHaveAttribute(
			"data-mobile-open",
			"false",
		);
	});

	it("opens the global search dialog from the top bar and keyboard shortcuts", () => {
		render(<AppLayout>Page Content</AppLayout>);

		fireEvent.click(screen.getByRole("button", { name: "Open Search" }));
		expect(screen.getByTestId("global-search-dialog")).toHaveAttribute(
			"data-open",
			"true",
		);

		fireEvent.keyDown(document, { key: "/" });
		expect(screen.getByTestId("global-search-dialog")).toHaveAttribute(
			"data-open",
			"true",
		);
	});

	it("opens search with Ctrl+K and prevents the browser shortcut", () => {
		render(<AppLayout>Page Content</AppLayout>);

		const prevented = !fireEvent.keyDown(document, {
			cancelable: true,
			ctrlKey: true,
			key: "k",
		});

		expect(prevented).toBe(true);
		expect(screen.getByTestId("global-search-dialog")).toHaveAttribute(
			"data-open",
			"true",
		);
	});

	it("ignores global shortcuts when the target should be skipped", () => {
		mockState.shouldIgnoreKeyboardTarget.mockReturnValue(true);

		render(<AppLayout>Page Content</AppLayout>);

		fireEvent.keyDown(document, { key: "/" });

		expect(screen.getByTestId("global-search-dialog")).toHaveAttribute(
			"data-open",
			"false",
		);
	});
});
