import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TrashBatchActionBar } from "@/components/trash/TrashBatchActionBar";

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, options?: Record<string, unknown>) => {
			if (key === "selected_count") return `selected:${options?.count}`;
			return key;
		},
	}),
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		"aria-label": ariaLabel,
		children,
		disabled,
		onClick,
		title,
	}: {
		"aria-label"?: string;
		children: React.ReactNode;
		disabled?: boolean;
		onClick?: () => void;
		title?: string;
	}) => (
		<button
			type="button"
			aria-label={ariaLabel}
			disabled={disabled}
			onClick={onClick}
			title={title}
		>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{`icon:${name}`}</span>,
}));

describe("TrashBatchActionBar", () => {
	it("does not render when nothing is selected", () => {
		const { container } = render(
			<TrashBatchActionBar
				count={0}
				onRestore={vi.fn()}
				onPurge={vi.fn()}
				onClearSelection={vi.fn()}
			/>,
		);

		expect(container).toBeEmptyDOMElement();
	});

	it("renders the selected count and triggers all batch actions", () => {
		const onRestore = vi.fn();
		const onPurge = vi.fn();
		const onClearSelection = vi.fn();

		render(
			<TrashBatchActionBar
				count={3}
				onRestore={onRestore}
				onPurge={onPurge}
				onClearSelection={onClearSelection}
			/>,
		);

		expect(screen.getByText("selected:3")).toBeInTheDocument();

		fireEvent.click(screen.getByText("files:trash_restore_selected"));
		fireEvent.click(screen.getByText("files:trash_delete_selected"));
		fireEvent.click(screen.getByRole("button", { name: "icon:X" }));

		expect(onRestore).toHaveBeenCalledTimes(1);
		expect(onPurge).toHaveBeenCalledTimes(1);
		expect(onClearSelection).toHaveBeenCalledTimes(1);
	});

	it("shows pending labels and disables actions while busy", () => {
		render(
			<TrashBatchActionBar
				count={2}
				pendingOperation="restore"
				onRestore={vi.fn()}
				onPurge={vi.fn()}
				onClearSelection={vi.fn()}
			/>,
		);

		const restoreButton = screen
			.getByText("files:trash_restoring")
			.closest("button");
		expect(restoreButton).toBeDisabled();
		const purgeButton = screen
			.getByText("files:trash_delete_selected")
			.closest("button");
		expect(purgeButton).toBeDisabled();
		expect(screen.getByRole("button", { name: "icon:X" })).toBeDisabled();
	});
});
