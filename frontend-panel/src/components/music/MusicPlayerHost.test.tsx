import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MusicPlayerHost } from "@/components/music/MusicPlayerHost";
import type { MusicPlaybackMode } from "@/stores/musicPlayerStore";

const mockState = vi.hoisted(() => ({
	clear: vi.fn(),
	closePanel: vi.fn(),
	parseMusicMetadataFromSource: vi.fn(),
	openPanel: vi.fn(),
	playNext: vi.fn(),
	playPrevious: vi.fn(),
	playTracks: vi.fn(),
	requestPlayback: vi.fn(),
	setError: vi.fn(),
	setPanelOpen: vi.fn(),
	setPlaybackMode: vi.fn(),
	setPlaybackRequested: vi.fn(),
	setPlaying: vi.fn(),
	updateTrackSource: vi.fn(),
	updateTrackMetadata: vi.fn(),
	state: {
		activeTrackId: null as string | null,
		error: null as string | null,
		isPanelOpen: false,
		isPlaying: false,
		playRequestVersion: 0,
		playRequested: false,
		playbackMode: "repeat_queue" as MusicPlaybackMode,
		queue: [] as Array<{
			expiresAt?: string;
			refreshStreamLink?: () => Promise<{
				expires_at: string;
				path: string;
			}>;
			id: string;
			metadata?: {
				artist?: string | null;
				artists?: string[] | null;
				artworkUrl?: string | null;
				title?: string | null;
			};
			mimeType: string;
			name: string;
			path: string;
			size?: number;
		}>,
	},
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
	}),
}));

vi.mock("@/stores/musicPlayerStore", () => ({
	useMusicPlayerStore: (
		selector: (state: {
			activeTrackId: typeof mockState.state.activeTrackId;
			clear: typeof mockState.clear;
			closePanel: typeof mockState.closePanel;
			error: string | null;
			isPanelOpen: boolean;
			isPlaying: boolean;
			openPanel: typeof mockState.openPanel;
			playNext: typeof mockState.playNext;
			playPrevious: typeof mockState.playPrevious;
			playRequestVersion: number;
			playRequested: boolean;
			playbackMode: MusicPlaybackMode;
			playTracks: typeof mockState.playTracks;
			queue: typeof mockState.state.queue;
			requestPlayback: typeof mockState.requestPlayback;
			setError: typeof mockState.setError;
			setPanelOpen: typeof mockState.setPanelOpen;
			setPlaybackMode: typeof mockState.setPlaybackMode;
			setPlaybackRequested: typeof mockState.setPlaybackRequested;
			setPlaying: typeof mockState.setPlaying;
			updateTrackMetadata: typeof mockState.updateTrackMetadata;
			updateTrackSource: typeof mockState.updateTrackSource;
		}) => unknown,
	) =>
		selector({
			...mockState.state,
			clear: mockState.clear,
			closePanel: mockState.closePanel,
			openPanel: mockState.openPanel,
			playNext: mockState.playNext,
			playPrevious: mockState.playPrevious,
			playTracks: mockState.playTracks,
			requestPlayback: mockState.requestPlayback,
			setError: mockState.setError,
			setPanelOpen: mockState.setPanelOpen,
			setPlaybackMode: mockState.setPlaybackMode,
			setPlaybackRequested: mockState.setPlaybackRequested,
			setPlaying: mockState.setPlaying,
			updateTrackMetadata: mockState.updateTrackMetadata,
			updateTrackSource: mockState.updateTrackSource,
		}),
}));

vi.mock("@/lib/musicPlayer", () => ({
	parseMusicMetadataFromSource: (...args: unknown[]) =>
		mockState.parseMusicMetadataFromSource(...args),
}));

function setQueue() {
	mockState.state.activeTrackId = "track-1";
	mockState.state.queue = [
		{
			id: "track-1",
			metadata: { artist: "Artist One", title: "Track One" },
			mimeType: "audio/mpeg",
			name: "track-one.mp3",
			path: "/files/7/download",
			size: 1024,
		},
		{
			id: "track-2",
			metadata: { artist: "Artist Two", title: "Track Two" },
			mimeType: "audio/mpeg",
			name: "track-two.mp3",
			path: "/files/8/download",
			size: 2048,
		},
	];
}

function mockOverflow(text: string, scrollWidth: number, clientWidth: number) {
	const textNode = [...document.querySelectorAll("*")].find(
		(element) => element.textContent === text,
	);
	if (!textNode) {
		throw new Error(`missing text node: ${text}`);
	}
	const viewport = textNode.parentElement;
	if (!viewport) {
		throw new Error(`missing viewport for text node: ${text}`);
	}
	Object.defineProperty(textNode, "scrollWidth", {
		configurable: true,
		value: scrollWidth,
	});
	Object.defineProperty(viewport, "clientWidth", {
		configurable: true,
		value: clientWidth,
	});
	window.dispatchEvent(new Event("resize"));
	return { textNode, viewport };
}

describe("MusicPlayerHost", () => {
	let originalResizeObserver: typeof ResizeObserver | undefined;

	beforeEach(() => {
		vi.useRealTimers();
		originalResizeObserver = window.ResizeObserver;
		class MockResizeObserver {
			callback: ResizeObserverCallback;

			constructor(callback: ResizeObserverCallback) {
				this.callback = callback;
			}

			disconnect = vi.fn();
			observe = vi.fn(() => {
				this.callback([], this as unknown as ResizeObserver);
			});
			unobserve = vi.fn();
		}
		window.ResizeObserver =
			MockResizeObserver as unknown as typeof ResizeObserver;
		vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
			callback(0);
			return 1;
		});
		vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => {});
		Object.defineProperty(HTMLMediaElement.prototype, "play", {
			configurable: true,
			value: vi.fn(() => Promise.resolve()),
		});
		Object.defineProperty(HTMLMediaElement.prototype, "pause", {
			configurable: true,
			value: vi.fn(),
		});
		mockState.clear.mockReset();
		mockState.closePanel.mockReset();
		mockState.openPanel.mockReset();
		mockState.parseMusicMetadataFromSource.mockReset();
		mockState.parseMusicMetadataFromSource.mockResolvedValue({
			artist: "Parsed Artist",
			title: "Parsed Title",
		});
		mockState.playNext.mockReset();
		mockState.playPrevious.mockReset();
		mockState.playTracks.mockReset();
		mockState.requestPlayback.mockReset();
		mockState.setError.mockReset();
		mockState.setPanelOpen.mockReset();
		mockState.setPlaybackMode.mockReset();
		mockState.setPlaybackRequested.mockReset();
		mockState.setPlaying.mockReset();
		mockState.updateTrackMetadata.mockReset();
		mockState.updateTrackSource.mockReset();
		mockState.state.activeTrackId = null;
		mockState.state.error = null;
		mockState.state.isPanelOpen = false;
		mockState.state.isPlaying = false;
		mockState.state.playRequestVersion = 0;
		mockState.state.playRequested = false;
		mockState.state.playbackMode = "repeat_queue";
		mockState.state.queue = [];
	});

	afterEach(() => {
		window.ResizeObserver = originalResizeObserver;
		vi.restoreAllMocks();
		vi.useRealTimers();
	});

	it("renders nothing when no track is loaded", () => {
		const { container } = render(<MusicPlayerHost />);

		expect(container).toBeEmptyDOMElement();
	});

	it("keeps the audio element loaded while the panel is collapsed", () => {
		setQueue();

		render(<MusicPlayerHost />);

		expect(document.querySelector("audio")).toHaveAttribute(
			"src",
			"/api/v1/files/7/download",
		);
		expect(screen.queryByText("Track One")).not.toBeInTheDocument();
	});

	it("renders the expanded player with track metadata and queue", () => {
		setQueue();
		mockState.state.isPanelOpen = true;

		render(<MusicPlayerHost />);

		expect(screen.getByText("music_player_title")).toBeInTheDocument();
		expect(screen.getByText("Track One")).toBeInTheDocument();
		expect(screen.getByText("Artist One")).toBeInTheDocument();
		expect(screen.getByText("Track Two")).toBeInTheDocument();
	});

	it("uses automatic marquee only for active overflowing music text", () => {
		mockState.state.activeTrackId = "track-1";
		mockState.state.isPanelOpen = true;
		mockState.state.queue = [
			{
				id: "track-1",
				metadata: {
					artist: "First Artist",
					artists: ["First Artist", "Second Artist"],
					title:
						"Very Long Track Title That Needs Automatic Scrolling In The Player",
				},
				mimeType: "audio/mpeg",
				name: "track-one.mp3",
				path: "/files/7/download",
				size: 1024,
			},
			{
				id: "track-2",
				metadata: {
					artist: "Other Artist",
					title: "Very Long Inactive Track Title That Must Stay Truncated",
				},
				mimeType: "audio/mpeg",
				name: "track-two.mp3",
				path: "/files/8/download",
				size: 2048,
			},
		];

		render(<MusicPlayerHost />);

		const { textNode: activeTitle, viewport: activeViewport } = mockOverflow(
			"Very Long Track Title That Needs Automatic Scrolling In The Player",
			720,
			240,
		);
		expect(activeViewport).toHaveAttribute("data-marquee-active", "true");
		expect(activeTitle).toHaveStyle({
			"--music-text-scroll-distance": "-480px",
		});
		expect(activeTitle).toHaveStyle({
			animation:
				"music-player-text-marquee 21.333333333333336s linear infinite",
		});
		expect(document.querySelector("style")?.textContent).toContain("12%");
		expect(document.querySelector("style")?.textContent).toContain("82%");

		const inactiveTitle = screen.getByText(
			"Very Long Inactive Track Title That Must Stay Truncated",
		);
		expect(inactiveTitle).toHaveClass("truncate");
		expect(inactiveTitle.parentElement).not.toHaveAttribute(
			"data-marquee-active",
			"true",
		);
		expect(
			screen.getAllByText("First Artist, Second Artist").length,
		).toBeGreaterThan(0);
	});

	it("can close or collapse the player panel", () => {
		setQueue();
		mockState.state.isPanelOpen = true;

		render(<MusicPlayerHost />);

		fireEvent.click(screen.getByRole("button", { name: "music_player_close" }));
		fireEvent.click(
			screen.getByRole("button", { name: "music_player_collapse" }),
		);

		expect(mockState.clear).toHaveBeenCalledTimes(1);
		expect(mockState.closePanel).toHaveBeenCalledTimes(1);
	});

	it("requests playback when play is clicked", () => {
		setQueue();
		mockState.state.isPanelOpen = true;

		render(<MusicPlayerHost />);

		fireEvent.click(screen.getByRole("button", { name: "music_player_play" }));

		expect(mockState.requestPlayback).toHaveBeenCalledTimes(1);
	});

	it("pauses playback when pause is clicked", () => {
		setQueue();
		mockState.state.isPanelOpen = true;
		mockState.state.isPlaying = true;
		mockState.state.playRequested = true;

		render(<MusicPlayerHost />);

		fireEvent.click(screen.getByRole("button", { name: "music_player_pause" }));

		expect(mockState.setPlaybackRequested).toHaveBeenCalledWith(false);
	});

	it("wires previous, next, playback mode, and queue item actions", () => {
		setQueue();
		mockState.state.isPanelOpen = true;

		render(<MusicPlayerHost />);

		fireEvent.click(
			screen.getByRole("button", { name: "music_player_previous" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "music_player_next" }));
		fireEvent.click(
			screen.getByRole("button", {
				name: "music_player_mode_repeat_queue",
			}),
		);
		fireEvent.click(screen.getByRole("button", { name: /Track Two/i }));

		expect(mockState.playPrevious).toHaveBeenCalledTimes(1);
		expect(mockState.playNext).toHaveBeenCalledTimes(1);
		expect(mockState.setPlaybackMode).toHaveBeenCalledWith("repeat_one");
		expect(mockState.playTracks).toHaveBeenCalledWith(
			mockState.state.queue,
			"track-2",
		);
	});

	it("reflects audio element events back into the player store", () => {
		setQueue();

		render(<MusicPlayerHost />);

		const audio = document.querySelector("audio");
		if (!audio) {
			throw new Error("audio element not found");
		}

		fireEvent.play(audio);
		expect(mockState.setError).toHaveBeenCalledWith(null);
		expect(mockState.setPlaying).toHaveBeenCalledWith(true);
		expect(mockState.setPlaybackRequested).toHaveBeenCalledWith(true);

		fireEvent.pause(audio);
		expect(mockState.setPlaying).toHaveBeenCalledWith(false);

		fireEvent.error(audio);
		expect(mockState.setError).toHaveBeenCalledWith("music_player_load_failed");
		expect(mockState.setPlaybackRequested).toHaveBeenCalledWith(false);
		expect(mockState.setPlaying).toHaveBeenCalledWith(false);
	});

	it("updates the seek control from audio metadata and lets users seek", () => {
		setQueue();
		mockState.state.isPanelOpen = true;

		render(<MusicPlayerHost />);

		const audio = document.querySelector("audio");
		if (!audio) {
			throw new Error("audio element not found");
		}
		Object.defineProperty(audio, "duration", {
			configurable: true,
			value: 120,
		});
		Object.defineProperty(audio, "currentTime", {
			configurable: true,
			writable: true,
			value: 30,
		});

		fireEvent.loadedMetadata(audio);
		fireEvent.timeUpdate(audio);

		const seek = screen.getByRole("slider", { name: "music_player_seek" });
		expect(seek).toHaveValue("25");

		fireEvent.change(seek, { target: { value: "50" } });

		expect(audio.currentTime).toBe(60);
		expect(seek).toHaveValue("50");
	});

	it("pauses while seeking and resumes only when it was previously playing", () => {
		setQueue();
		mockState.state.isPanelOpen = true;
		mockState.state.isPlaying = true;
		mockState.state.playRequested = true;

		render(<MusicPlayerHost />);

		const audio = document.querySelector("audio");
		if (!audio) {
			throw new Error("audio element not found");
		}
		Object.defineProperty(audio, "duration", {
			configurable: true,
			value: 120,
		});
		fireEvent.loadedMetadata(audio);

		const seek = screen.getByRole("slider", { name: "music_player_seek" });
		fireEvent.pointerDown(seek);
		fireEvent.change(seek, { target: { value: "50" } });
		fireEvent.pointerUp(seek);

		expect(HTMLMediaElement.prototype.pause).toHaveBeenCalledTimes(1);
		expect(mockState.setPlaybackRequested).toHaveBeenCalledWith(false);
		expect(mockState.requestPlayback).toHaveBeenCalledTimes(1);
		expect(audio.currentTime).toBe(60);
	});

	it("parses metadata once per track id so metadata updates do not re-fetch the same range", async () => {
		setQueue();
		mockState.state.isPanelOpen = true;

		const { rerender } = render(<MusicPlayerHost />);

		await act(async () => {
			await Promise.resolve();
		});

		const [firstTrack, secondTrack] = mockState.state.queue;
		if (!firstTrack || !secondTrack) {
			throw new Error("expected queued test tracks");
		}
		mockState.state.queue = [
			{
				...firstTrack,
				metadata: {
					artist: "Parsed Artist",
					artworkUrl: "data:image/jpeg;base64,cover",
					title: "Parsed Title",
				},
			},
			secondTrack,
		];
		rerender(<MusicPlayerHost />);

		await act(async () => {
			await Promise.resolve();
		});

		expect(mockState.parseMusicMetadataFromSource).toHaveBeenCalledTimes(1);
		expect(mockState.updateTrackMetadata).toHaveBeenCalledWith("track-1", {
			artist: "Parsed Artist",
			title: "Parsed Title",
		});
	});

	it("moves to the next track when the current track ends", () => {
		setQueue();

		render(<MusicPlayerHost />);

		const audio = document.querySelector("audio");
		if (!audio) {
			throw new Error("audio element not found");
		}

		fireEvent.ended(audio);

		expect(mockState.playNext).toHaveBeenCalledTimes(1);
	});

	it("refreshes expiring stream sessions before the current link expires", async () => {
		vi.useFakeTimers();
		const refreshStreamLink = vi.fn(async () => ({
			expires_at: "2026-01-01T03:00:00Z",
			path: "/api/v1/s/share-token/stream/session-2/track.mp3",
		}));
		vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
		mockState.state.activeTrackId = "track-1";
		mockState.state.queue = [
			{
				expiresAt: "2026-01-01T00:03:00Z",
				id: "track-1",
				mimeType: "audio/mpeg",
				name: "track.mp3",
				path: "/api/v1/s/share-token/stream/session-1/track.mp3",
				refreshStreamLink,
			},
		];

		render(<MusicPlayerHost />);

		await act(async () => {
			vi.advanceTimersByTime(60_000);
			await Promise.resolve();
		});

		expect(refreshStreamLink).toHaveBeenCalledTimes(1);
		expect(mockState.updateTrackSource).toHaveBeenCalledWith("track-1", {
			expires_at: "2026-01-01T03:00:00Z",
			path: "/api/v1/s/share-token/stream/session-2/track.mp3",
		});
	});
});
