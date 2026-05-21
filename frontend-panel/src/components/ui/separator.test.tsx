import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Separator } from "@/components/ui/separator";

describe("Separator", () => {
	it("renders a horizontal separator by default", () => {
		render(<Separator className="custom-separator" />);

		const separator = screen.getByRole("separator");

		expect(separator).toHaveAttribute("data-slot", "separator");
		expect(separator).toHaveAttribute("data-orientation", "horizontal");
		expect(separator).toHaveClass("shrink-0", "bg-border", "custom-separator");
	});

	it("renders a vertical separator when requested", () => {
		render(<Separator orientation="vertical" />);

		expect(screen.getByRole("separator")).toHaveAttribute(
			"data-orientation",
			"vertical",
		);
	});
});
