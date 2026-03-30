import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { AsterDriveWordmark } from "@/components/common/AsterDriveWordmark";
import { useThemeStore } from "@/stores/themeStore";

describe("AsterDriveWordmark", () => {
	beforeEach(() => {
		document.documentElement.classList.remove("dark");
		useThemeStore.setState({ resolvedTheme: "light" });
	});

	it("uses the dark wordmark on light theme", () => {
		render(<AsterDriveWordmark alt="AsterDrive" />);

		expect(screen.getByRole("img", { name: "AsterDrive" })).toHaveAttribute(
			"src",
			"/static/asterdrive/asterdrive-dark.svg",
		);
	});

	it("uses the light wordmark on dark theme", () => {
		useThemeStore.setState({ resolvedTheme: "dark" });

		render(<AsterDriveWordmark alt="AsterDrive" />);

		expect(screen.getByRole("img", { name: "AsterDrive" })).toHaveAttribute(
			"src",
			"/static/asterdrive/asterdrive-light.svg",
		);
	});
});
