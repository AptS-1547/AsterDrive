import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AudioPreview } from "@/components/files/preview/AudioPreview";

const mockState = vi.hoisted(() => ({
	useBlobUrl: vi.fn(),
	warn: vi.fn(),
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
	}),
}));

vi.mock("@/hooks/useBlobUrl", () => ({
	useBlobUrl: (...args: unknown[]) => mockState.useBlobUrl(...args),
}));

vi.mock("@/lib/logger", () => ({
	logger: {
		warn: (...args: unknown[]) => mockState.warn(...args),
	},
}));

describe("AudioPreview", () => {
	beforeEach(() => {
		mockState.useBlobUrl.mockReset();
		mockState.warn.mockReset();
	});

	it("passes the HTTP download URL directly to the audio element", () => {
		render(
			<AudioPreview
				file={{ name: "track.mp3", mime_type: "audio/mpeg" }}
				path="/files/7/download"
			/>,
		);

		expect(mockState.useBlobUrl).not.toHaveBeenCalled();
		expect(document.querySelector("audio")).toHaveAttribute(
			"src",
			"/api/v1/files/7/download",
		);
		expect(document.querySelector("audio")).toHaveAttribute(
			"preload",
			"metadata",
		);
	});

	it("keeps already public preview URLs unchanged", () => {
		render(
			<AudioPreview
				file={{ name: "track.mp3", mime_type: "audio/mpeg" }}
				path="/pv/token/track.mp3"
			/>,
		);

		expect(document.querySelector("audio")).toHaveAttribute(
			"src",
			"/pv/token/track.mp3",
		);
	});

	it("creates a stream session before rendering the audio source when provided", async () => {
		const mediaStreamLinkFactory = vi.fn(async () => ({
			expires_at: "2026-01-01T00:00:00Z",
			path: "/api/v1/s/share-token/stream/session-token/track.mp3",
		}));

		render(
			<AudioPreview
				file={{ name: "track.mp3", mime_type: "audio/mpeg" }}
				path="/s/share-token/download"
				mediaStreamLinkFactory={mediaStreamLinkFactory}
			/>,
		);

		expect(screen.getByText("loading_preview")).toBeInTheDocument();
		expect(document.querySelector("audio")).toBeNull();

		await waitFor(() => {
			expect(document.querySelector("audio")).toHaveAttribute(
				"src",
				"/api/v1/s/share-token/stream/session-token/track.mp3",
			);
		});
		expect(mediaStreamLinkFactory).toHaveBeenCalledTimes(1);
	});

	it("renders the preview error when stream session creation fails", async () => {
		const mediaStreamLinkFactory = vi.fn(async () => {
			throw new Error("session failed");
		});

		render(
			<AudioPreview
				file={{ name: "track.mp3", mime_type: "audio/mpeg" }}
				path="/s/share-token/download"
				mediaStreamLinkFactory={mediaStreamLinkFactory}
			/>,
		);

		expect(await screen.findByText("preview_load_failed")).toBeInTheDocument();
		expect(mockState.warn).toHaveBeenCalledWith(
			"audio stream session creation failed",
			"track.mp3",
			expect.any(Error),
		);
	});

	it("renders the preview error when the audio element fails to load", () => {
		render(
			<AudioPreview
				file={{ name: "track.mp3", mime_type: "audio/mpeg" }}
				path="/files/7/download"
			/>,
		);

		const audio = document.querySelector("audio");
		if (!audio) {
			throw new Error("audio element not found");
		}

		fireEvent.error(audio);

		expect(screen.getByText("preview_load_failed")).toBeInTheDocument();
	});
});
