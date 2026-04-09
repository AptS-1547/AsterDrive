import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FileTypeIcon } from "@/components/files/FileTypeIcon";

vi.mock("@/components/files/preview/file-capabilities", () => ({
	getFileTypeInfo: vi.fn(() => ({
		icon: "FileText",
		color: "text-blue-500",
	})),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name, className }: { name: string; className?: string }) => (
		<span data-testid="icon" data-name={name} className={className} />
	),
}));

const mockHasLanguageIcon = vi.fn(() => false);
const mockIsIconMapLoaded = vi.fn(() => true);
const mockLoadLanguageIcons = vi.fn(() => Promise.resolve());

vi.mock("@/components/ui/language-icon", () => ({
	hasLanguageIcon: (name: string) => mockHasLanguageIcon(name),
	isIconMapLoaded: () => mockIsIconMapLoaded(),
	loadLanguageIcons: () => mockLoadLanguageIcons(),
	LanguageIcon: ({ name, className }: { name: string; className?: string }) => (
		<span data-testid="language-icon" data-name={name} className={className} />
	),
}));

beforeEach(() => {
	mockHasLanguageIcon.mockReset();
	mockHasLanguageIcon.mockReturnValue(false);
	mockIsIconMapLoaded.mockReset();
	mockIsIconMapLoaded.mockReturnValue(true);
	mockLoadLanguageIcons.mockClear();
});

describe("FileTypeIcon", () => {
	it("renders the icon and color returned by file type detection", () => {
		render(
			<FileTypeIcon
				mimeType="application/pdf"
				fileName="manual.pdf"
				className="h-4 w-4"
			/>,
		);

		expect(screen.getByTestId("icon")).toHaveAttribute("data-name", "FileText");
		expect(screen.getByTestId("icon")).toHaveClass(
			"text-blue-500",
			"h-4",
			"w-4",
		);
	});

	it("renders a language icon when the icon map is loaded and the file matches", () => {
		mockHasLanguageIcon.mockReturnValue(true);

		render(
			<FileTypeIcon
				mimeType="text/plain"
				fileName="main.ts"
				className="h-4 w-4"
			/>,
		);

		expect(screen.getByTestId("language-icon")).toHaveAttribute(
			"data-name",
			"main.ts",
		);
		expect(screen.getByTestId("language-icon")).toHaveClass("h-4", "w-4");
		expect(screen.queryByTestId("icon")).not.toBeInTheDocument();
	});
});
