import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ItemCheckbox } from "@/components/ui/item-checkbox";

describe("ItemCheckbox", () => {
	it("renders unchecked state without the check icon", () => {
		const onChange = vi.fn();
		const { container } = render(
			<ItemCheckbox
				checked={false}
				onChange={onChange}
				className="custom-checkbox"
			/>,
		);

		const checkbox = screen.getByRole("button");

		expect(checkbox).toHaveAttribute("aria-pressed", "false");
		expect(checkbox).toHaveAttribute("tabindex", "-1");
		expect(checkbox).toHaveClass("custom-checkbox");
		expect(checkbox).not.toHaveAttribute("data-drag-preview-hidden");
		expect(container.querySelector("svg")).not.toBeInTheDocument();
	});

	it("renders checked state and calls its change handler", () => {
		const onChange = vi.fn();
		const { container } = render(
			<ItemCheckbox
				checked={true}
				onChange={onChange}
				data-drag-preview-hidden={true}
			/>,
		);

		const checkbox = screen.getByRole("button", { pressed: true });
		fireEvent.click(checkbox);

		expect(checkbox).toHaveAttribute("aria-pressed", "true");
		expect(checkbox).toHaveAttribute("data-drag-preview-hidden", "true");
		expect(container.querySelector("svg")).toBeInTheDocument();
		expect(onChange).toHaveBeenCalledTimes(1);
	});
});
