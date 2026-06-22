import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PreviewUnavailable } from "@/components/files/preview/shared/PreviewUnavailable";

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
	}),
}));

describe("PreviewUnavailable", () => {
	it("renders the translated unavailable title and description", () => {
		render(<PreviewUnavailable />);

		expect(screen.getByText("preview_not_available")).toBeInTheDocument();
		expect(screen.getByText("preview_not_available_desc")).toBeInTheDocument();
	});
});
