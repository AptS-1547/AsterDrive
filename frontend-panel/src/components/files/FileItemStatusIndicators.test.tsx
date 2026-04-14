import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FileItemStatusIndicators } from "@/components/files/FileItemStatusIndicators";

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => `translated:${key}`,
	}),
}));

vi.mock("@/components/ui/badge", () => ({
	Badge: ({
		children,
		className,
		title,
	}: {
		children: React.ReactNode;
		className?: string;
		title?: string;
	}) => (
		<span data-testid="badge" className={className} title={title}>
			{children}
		</span>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => (
		<span data-testid="icon" data-name={name} />
	),
}));

describe("FileItemStatusIndicators", () => {
	it("renders nothing when no status flags are active", () => {
		const { container } = render(<FileItemStatusIndicators />);

		expect(container).toBeEmptyDOMElement();
	});

	it("renders shared and locked badges with translated labels", () => {
		render(<FileItemStatusIndicators isShared isLocked />);

		expect(screen.getByText("translated:share")).toBeInTheDocument();
		expect(screen.getByText("translated:lock")).toBeInTheDocument();
		expect(screen.getAllByTestId("badge")).toHaveLength(2);
		expect(screen.getAllByTestId("icon")[0]).toHaveAttribute(
			"data-name",
			"LinkSimple",
		);
		expect(screen.getAllByTestId("icon")[1]).toHaveAttribute(
			"data-name",
			"Lock",
		);
	});

	it("uses compact badge styling when requested", () => {
		render(<FileItemStatusIndicators isShared compact />);

		expect(screen.queryByTestId("badge")).not.toBeInTheDocument();
		expect(screen.getByTitle("translated:share")).toHaveClass(
			"size-5",
			"rounded-full",
		);
	});
});
