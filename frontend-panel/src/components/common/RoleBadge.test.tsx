import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RoleBadge } from "@/components/common/RoleBadge";
import { getRoleBadgeClass } from "@/components/common/userBadgeClasses";

vi.mock("@/components/ui/badge", () => ({
	Badge: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => (
		<span data-testid="badge" className={className}>
			{children}
		</span>
	),
}));

describe("RoleBadge", () => {
	it("returns the admin badge class for admin users", () => {
		expect(getRoleBadgeClass("admin" as never)).toContain("text-blue-600");
		expect(getRoleBadgeClass("user" as never)).toContain(
			"text-muted-foreground",
		);
	});

	it("renders the provided label with the computed role class", () => {
		render(<RoleBadge role={"admin" as never} label="Administrator" />);

		expect(screen.getByText("Administrator")).toBeInTheDocument();
		expect(screen.getByTestId("badge")).toHaveClass(
			"border-blue-500/60",
			"bg-blue-500/10",
		);
	});
});
