import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { UserStatusBadge } from "@/components/common/UserStatusBadge";
import {
	getRoleBadgeClass,
	getStatusBadgeClass,
} from "@/components/common/userBadgeClasses";

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

describe("UserStatusBadge", () => {
	it("returns distinct classes for role and status variants", () => {
		expect(getRoleBadgeClass("admin" as never)).toContain("text-blue-600");
		expect(getRoleBadgeClass("user" as never)).toContain(
			"text-muted-foreground",
		);
		expect(getStatusBadgeClass("active" as never)).toContain("text-green-600");
		expect(getStatusBadgeClass("disabled" as never)).toContain(
			"text-amber-600",
		);
	});

	it("renders the provided status label with the computed status class", () => {
		render(<UserStatusBadge status={"active" as never} label="Active" />);

		expect(screen.getByText("Active")).toBeInTheDocument();
		expect(screen.getByTestId("badge")).toHaveClass(
			"border-green-500/60",
			"bg-green-500/10",
		);
	});
});
