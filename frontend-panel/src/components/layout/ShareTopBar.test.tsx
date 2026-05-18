import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ShareTopBar } from "@/components/layout/ShareTopBar";

const mockState = vi.hoisted(() => ({
	music: {
		isPlaying: false,
		queue: [] as Array<{ id: string }>,
		togglePanel: vi.fn(),
	},
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => `translated:${key}`,
	}),
}));

vi.mock("@/components/layout/TopBarShell", () => ({
	TopBarShell: ({
		left,
		right,
		heightClassName,
	}: {
		left: React.ReactNode;
		right: React.ReactNode;
		heightClassName?: string;
	}) => (
		<div data-testid="share-topbar-shell" data-height={heightClassName}>
			<div>{left}</div>
			<div>{right}</div>
		</div>
	),
}));

vi.mock("@/stores/musicPlayerStore", () => ({
	useMusicPlayerStore: (selector: (state: typeof mockState.music) => unknown) =>
		selector(mockState.music),
}));

describe("ShareTopBar", () => {
	beforeEach(() => {
		mockState.music.isPlaying = false;
		mockState.music.queue = [];
		mockState.music.togglePanel.mockReset();
	});

	it("renders a compact public-share top bar", () => {
		render(<ShareTopBar />);

		expect(screen.getByAltText("translated:app_name")).toBeInTheDocument();
		expect(screen.getByTestId("share-topbar-shell")).toHaveAttribute(
			"data-height",
			"h-14",
		);
		expect(screen.getByText("translated:files:share")).toHaveClass("sr-only");
	});

	it("toggles the music player when music is queued", () => {
		mockState.music.queue = [{ id: "track-1" }];

		render(<ShareTopBar />);

		fireEvent.click(
			screen.getByRole("button", {
				name: "translated:files:music_player_open",
			}),
		);

		expect(mockState.music.togglePanel).toHaveBeenCalledTimes(1);
	});
});
