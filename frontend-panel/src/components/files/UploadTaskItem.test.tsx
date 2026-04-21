import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { UploadTaskItem } from "@/components/files/UploadTaskItem";

vi.mock("@/components/ui/badge", () => ({
	Badge: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => <span className={className}>{children}</span>,
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		onClick,
		title,
	}: {
		children: React.ReactNode;
		onClick?: () => void;
		title?: string;
	}) => (
		<button type="button" onClick={onClick} aria-label={title}>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => (
		<span data-testid="icon" data-name={name} />
	),
}));

vi.mock("@/components/ui/progress", () => ({
	Progress: ({ value, className }: { value: number; className?: string }) => (
		<div data-testid="progress" data-value={value} className={className} />
	),
}));

describe("UploadTaskItem", () => {
	it("renders task metadata and falls back to status when detail is absent", () => {
		const { container } = render(
			<UploadTaskItem
				title="report.pdf"
				status="Uploading"
				mode="Chunked"
				progress={45}
			/>,
		);

		expect(screen.getByText("report.pdf")).toBeInTheDocument();
		expect(screen.getByText("Chunked")).toBeInTheDocument();
		expect(screen.getByText("Uploading")).toBeInTheDocument();
		expect(screen.getByText("45%")).toBeInTheDocument();
		expect(screen.getByTestId("progress")).toHaveAttribute("data-value", "45");
		expect(container.firstChild).toHaveClass("bg-background/60");
	});

	it("renders completed styling, custom detail text, and action callbacks", () => {
		const onRetry = vi.fn();
		const onCancel = vi.fn();
		const { container } = render(
			<UploadTaskItem
				title="archive.zip"
				status="Failed"
				mode="Presigned"
				progress={100}
				detail="Retry required"
				completed
				actions={[
					{ icon: "ArrowsClockwise", label: "Retry", onClick: onRetry },
					{ icon: "X", label: "Cancel", onClick: onCancel },
				]}
			/>,
		);

		expect(screen.getByText("Retry required")).toBeInTheDocument();
		expect(container.firstChild).toHaveClass("bg-muted/30", "opacity-80");
		expect(screen.getAllByTestId("icon")[0]).toHaveAttribute(
			"data-name",
			"ArrowsClockwise",
		);
		expect(screen.getAllByTestId("icon")[1]).toHaveAttribute("data-name", "X");

		fireEvent.click(screen.getByRole("button", { name: "Retry" }));
		fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

		expect(onRetry).toHaveBeenCalledTimes(1);
		expect(onCancel).toHaveBeenCalledTimes(1);
	});
});
