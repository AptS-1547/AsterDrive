import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { OpenWithChooser } from "@/components/files/preview/OpenWithChooser";

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => `translated:${key}`,
	}),
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({ children }: { children?: React.ReactNode }) => (
		<button type="button">{children}</button>
	),
}));

vi.mock("@/components/ui/dropdown-menu", () => ({
	DropdownMenu: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DropdownMenuTrigger: ({ children }: { children: React.ReactNode }) => (
		<button type="button">{children}</button>
	),
	DropdownMenuContent: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DropdownMenuItem: ({
		children,
		onClick,
	}: {
		children: React.ReactNode;
		onClick?: () => void;
	}) => (
		<button type="button" data-testid="menu-item" onClick={onClick}>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => (
		<span data-testid="icon" data-name={name} />
	),
}));

describe("OpenWithChooser", () => {
	it("returns null when there are not enough options to choose from", () => {
		const { container } = render(
			<OpenWithChooser
				options={[{ icon: "TextT", labelKey: "mode_code", mode: "code" }]}
				value="code"
				onChange={vi.fn()}
			/>,
		);

		expect(container).toBeEmptyDOMElement();
	});

	it("uses the selected option in the trigger and marks the active menu item", () => {
		const onChange = vi.fn();

		render(
			<OpenWithChooser
				options={[
					{ icon: "TextT", labelKey: "mode_code", mode: "code" },
					{
						icon: "MarkdownLogo",
						labelKey: "mode_markdown",
						mode: "markdown",
					},
				]}
				value="markdown"
				onChange={onChange}
			/>,
		);

		expect(screen.getAllByTestId("icon")[0]).toHaveAttribute(
			"data-name",
			"MarkdownLogo",
		);
		expect(screen.getAllByText("translated:mode_markdown")).toHaveLength(2);
		expect(
			screen
				.getAllByTestId("icon")
				.some((element) => element.getAttribute("data-name") === "Check"),
		).toBe(true);

		fireEvent.click(screen.getAllByTestId("menu-item")[0]);

		expect(onChange).toHaveBeenCalledWith("code");
	});

	it("falls back to the first option when the provided value is missing", () => {
		render(
			<OpenWithChooser
				options={[
					{ icon: "TextT", labelKey: "mode_code", mode: "code" },
					{
						icon: "MarkdownLogo",
						labelKey: "mode_markdown",
						mode: "markdown",
					},
				]}
				value={"formatted" as never}
				onChange={vi.fn()}
			/>,
		);

		expect(screen.getAllByTestId("icon")[0]).toHaveAttribute(
			"data-name",
			"TextT",
		);
		expect(screen.getAllByText("translated:mode_code")).toHaveLength(2);
	});

	it("prefers a custom option label when provided", () => {
		render(
			<OpenWithChooser
				options={[
					{
						icon: "Globe",
						label: "Jellyfin",
						labelKey: "ignored",
						mode: "video",
					},
					{ icon: "Monitor", labelKey: "mode_native", mode: "videoBrowser" },
				]}
				value="video"
				onChange={vi.fn()}
			/>,
		);

		expect(screen.getAllByText("Jellyfin")).toHaveLength(2);
		expect(screen.queryByText("translated:ignored")).not.toBeInTheDocument();
	});
});
