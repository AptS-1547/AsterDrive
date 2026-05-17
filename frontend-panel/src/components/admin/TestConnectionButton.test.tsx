import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TestConnectionButton } from "@/components/admin/TestConnectionButton";

const mockWarn = vi.hoisted(() => vi.fn());

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
	}),
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		disabled,
		onClick,
		type,
	}: {
		children: React.ReactNode;
		disabled?: boolean;
		onClick?: React.MouseEventHandler<HTMLButtonElement>;
		type?: "button" | "submit";
	}) => (
		<button type={type ?? "button"} disabled={disabled} onClick={onClick}>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{name}</span>,
}));

vi.mock("@/lib/logger", () => ({
	logger: {
		warn: (...args: unknown[]) => mockWarn(...args),
	},
}));

describe("TestConnectionButton", () => {
	it("shows a failed result and logs when the test promise rejects", async () => {
		const error = new Error("probe failed");
		render(<TestConnectionButton onTest={vi.fn().mockRejectedValue(error)} />);

		fireEvent.click(screen.getByRole("button"));

		await waitFor(() => {
			expect(mockWarn).toHaveBeenCalledWith("connection test failed", error);
		});
		expect(screen.getByText("WifiHigh")).toBeInTheDocument();
	});
});
