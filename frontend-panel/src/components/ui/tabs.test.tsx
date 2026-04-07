import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

describe("TabsList", () => {
	it("keeps default tabs sized to their content", () => {
		render(
			<Tabs defaultValue="overview">
				<TabsList>
					<TabsTrigger value="overview">Overview</TabsTrigger>
					<TabsTrigger value="members">Members</TabsTrigger>
				</TabsList>
			</Tabs>,
		);

		const list = screen.getByRole("tablist");

		expect(list).toHaveClass("inline-flex", "w-fit", "bg-muted");
		expect(list).not.toHaveClass("w-full");
	});

	it("stretches line tabs to the available width", () => {
		render(
			<Tabs defaultValue="overview">
				<TabsList variant="line">
					<TabsTrigger value="overview">Overview</TabsTrigger>
					<TabsTrigger value="members">Members</TabsTrigger>
				</TabsList>
			</Tabs>,
		);

		const list = screen.getByRole("tablist");

		expect(list).toHaveClass("flex", "min-w-0", "w-full", "max-w-full");
		expect(list).not.toHaveClass("w-fit");
	});
});
