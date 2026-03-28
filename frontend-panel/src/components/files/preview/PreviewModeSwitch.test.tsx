import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PreviewModeSwitch } from "@/components/files/preview/PreviewModeSwitch";

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => `translated:${key}`,
	}),
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		onClick,
		className,
	}: {
		children: React.ReactNode;
		onClick?: () => void;
		className?: string;
	}) => (
		<button type="button" onClick={onClick} className={className}>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => (
		<span data-testid="icon" data-name={name} />
	),
}));

describe("PreviewModeSwitch", () => {
	it("returns null when there is only one available option", () => {
		const { container } = render(
			<PreviewModeSwitch
				options={[{ icon: "TextT", labelKey: "mode_code", mode: "code" }]}
				value="code"
				onChange={vi.fn()}
			/>,
		);

		expect(container).toBeEmptyDOMElement();
	});

	it("renders all options, highlights the active one, and emits mode changes", () => {
		const onChange = vi.fn();

		render(
			<PreviewModeSwitch
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

		expect(screen.getByText("translated:mode_code")).toBeInTheDocument();
		expect(screen.getByText("translated:mode_markdown")).toBeInTheDocument();
		expect(screen.getAllByTestId("icon")[0]).toHaveAttribute(
			"data-name",
			"TextT",
		);
		expect(screen.getAllByTestId("icon")[1]).toHaveAttribute(
			"data-name",
			"MarkdownLogo",
		);
		expect(
			screen.getByRole("button", { name: /translated:mode_markdown/i }),
		).toHaveClass("bg-accent", "text-foreground");

		fireEvent.click(
			screen.getByRole("button", { name: /translated:mode_code/i }),
		);

		expect(onChange).toHaveBeenCalledWith("code");
	});

	it("renders a custom label without translation when provided", () => {
		render(
			<PreviewModeSwitch
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

		expect(screen.getByText("Jellyfin")).toBeInTheDocument();
		expect(screen.queryByText("translated:ignored")).not.toBeInTheDocument();
	});
});
