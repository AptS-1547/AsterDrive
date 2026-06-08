import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { UnsavedChangesGuard } from "@/components/files/preview/UnsavedChangesGuard";

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => `translated:${key}`,
	}),
}));

describe("UnsavedChangesGuard", () => {
	it("renders an inline discard guard without opening a nested dialog", () => {
		const onOpenChange = vi.fn();
		const onConfirm = vi.fn();

		render(
			<UnsavedChangesGuard
				open
				onOpenChange={onOpenChange}
				onConfirm={onConfirm}
			/>,
		);

		expect(screen.getByText("translated:are_you_sure")).toBeInTheDocument();
		expect(
			screen.getByText("translated:files:unsaved_confirm_desc"),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", {
				name: "translated:files:discard_changes",
			}),
		).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "translated:cancel" }));
		fireEvent.click(
			screen.getByRole("button", {
				name: "translated:files:discard_changes",
			}),
		);

		expect(onOpenChange).toHaveBeenCalledWith(false);
		expect(onConfirm).toHaveBeenCalledTimes(1);
	});
});
