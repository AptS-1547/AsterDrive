import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
	Progress,
	ProgressLabel,
	ProgressValue,
} from "@/components/ui/progress";

describe("Progress", () => {
	it("renders progress structure with label and value slots", () => {
		const { container } = render(
			<Progress value={42} className="custom-progress">
				<ProgressLabel className="custom-label">Upload</ProgressLabel>
				<ProgressValue className="custom-value">42%</ProgressValue>
			</Progress>,
		);

		const root = container.querySelector("[data-slot='progress']");
		const track = container.querySelector("[data-slot='progress-track']");
		const indicator = container.querySelector(
			"[data-slot='progress-indicator']",
		);
		const label = screen.getByText("Upload");
		const value = screen.getByText("42%");

		expect(root).toHaveClass("flex", "flex-wrap", "custom-progress");
		expect(track).toHaveAttribute("data-theme-surface", "meter");
		expect(indicator).toHaveAttribute("data-theme-surface", "meter");
		expect(label).toHaveAttribute("data-slot", "progress-label");
		expect(label).toHaveClass("text-sm", "custom-label");
		expect(value).toHaveAttribute("data-slot", "progress-value");
		expect(value).toHaveClass("tabular-nums", "custom-value");
	});
});
