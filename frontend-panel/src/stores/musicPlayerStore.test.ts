import { beforeEach, describe, expect, it, vi } from "vitest";

async function loadMusicPlayerStore() {
	vi.resetModules();
	return await import("@/stores/musicPlayerStore");
}

describe("musicPlayerStore", () => {
	beforeEach(() => {
		vi.useRealTimers();
	});

	it("starts playback for a new track and records a play request version", async () => {
		const { useMusicPlayerStore } = await loadMusicPlayerStore();

		useMusicPlayerStore.getState().playTrack({
			id: "track-1",
			mimeType: "audio/mpeg",
			name: "track.mp3",
			path: "/files/7/download",
			size: 4096,
		});

		expect(useMusicPlayerStore.getState()).toMatchObject({
			activeTrackId: "track-1",
			error: null,
			playRequested: true,
			playRequestVersion: 1,
			queue: [
				{
					id: "track-1",
					mimeType: "audio/mpeg",
					name: "track.mp3",
					path: "/files/7/download",
					size: 4096,
				},
			],
		});
	});

	it("loads a queue and starts the requested active track", async () => {
		const { useMusicPlayerStore } = await loadMusicPlayerStore();

		useMusicPlayerStore.getState().playTracks(
			[
				{
					id: "track-1",
					mimeType: "audio/mpeg",
					name: "one.mp3",
					path: "/files/1/download",
				},
				{
					id: "track-2",
					mimeType: "audio/mpeg",
					name: "two.mp3",
					path: "/files/2/download",
				},
			],
			"track-2",
		);

		expect(useMusicPlayerStore.getState()).toMatchObject({
			activeTrackId: "track-2",
			playRequested: true,
			queue: [
				expect.objectContaining({ id: "track-1" }),
				expect.objectContaining({ id: "track-2" }),
			],
		});
	});

	it("deduplicates queued tracks by id", async () => {
		const { useMusicPlayerStore } = await loadMusicPlayerStore();

		useMusicPlayerStore.getState().playTracks(
			[
				{
					id: "track-1",
					mimeType: "audio/mpeg",
					name: "one.mp3",
					path: "/files/1/download",
				},
				{
					id: "track-1",
					mimeType: "audio/mpeg",
					name: "duplicate.mp3",
					path: "/files/1-duplicate/download",
				},
			],
			"track-1",
		);

		expect(useMusicPlayerStore.getState().queue).toHaveLength(1);
		expect(useMusicPlayerStore.getState().queue[0]).toMatchObject({
			name: "one.mp3",
		});
	});

	it("increments the play request version when playback is requested again", async () => {
		const { useMusicPlayerStore } = await loadMusicPlayerStore();

		useMusicPlayerStore.getState().playTrack({
			id: "track-1",
			mimeType: "audio/mpeg",
			name: "track.mp3",
			path: "/files/7/download",
		});
		useMusicPlayerStore.getState().setPlaybackRequested(false);
		useMusicPlayerStore.getState().requestPlayback();

		expect(useMusicPlayerStore.getState()).toMatchObject({
			playRequested: true,
			playRequestVersion: 2,
		});
	});

	it("moves through the queue and wraps in repeat queue mode", async () => {
		const { useMusicPlayerStore } = await loadMusicPlayerStore();

		useMusicPlayerStore.getState().playTracks(
			[
				{
					id: "track-1",
					mimeType: "audio/mpeg",
					name: "one.mp3",
					path: "/files/1/download",
				},
				{
					id: "track-2",
					mimeType: "audio/mpeg",
					name: "two.mp3",
					path: "/files/2/download",
				},
			],
			"track-2",
		);
		useMusicPlayerStore.getState().playNext();

		expect(useMusicPlayerStore.getState().activeTrackId).toBe("track-1");

		useMusicPlayerStore.getState().playPrevious();

		expect(useMusicPlayerStore.getState().activeTrackId).toBe("track-2");
	});

	it("keeps the active track when repeat one is enabled", async () => {
		const { useMusicPlayerStore } = await loadMusicPlayerStore();

		useMusicPlayerStore.getState().playTracks(
			[
				{
					id: "track-1",
					mimeType: "audio/mpeg",
					name: "one.mp3",
					path: "/files/1/download",
				},
				{
					id: "track-2",
					mimeType: "audio/mpeg",
					name: "two.mp3",
					path: "/files/2/download",
				},
			],
			"track-1",
		);
		useMusicPlayerStore.getState().setPlaybackMode("repeat_one");
		useMusicPlayerStore.getState().playNext();

		expect(useMusicPlayerStore.getState().activeTrackId).toBe("track-1");
	});

	it("updates only the matching queued track source after a stream session refresh", async () => {
		const { useMusicPlayerStore } = await loadMusicPlayerStore();

		useMusicPlayerStore.getState().playTracks(
			[
				{
					expiresAt: "2026-01-01T00:30:00Z",
					id: "track-1",
					mimeType: "audio/mpeg",
					name: "one.mp3",
					path: "/api/v1/s/share-token/stream/session-1/one.mp3",
				},
				{
					expiresAt: "2026-01-01T00:30:00Z",
					id: "track-2",
					mimeType: "audio/mpeg",
					name: "two.mp3",
					path: "/api/v1/s/share-token/stream/session-1/two.mp3",
				},
			],
			"track-1",
		);

		useMusicPlayerStore.getState().updateTrackSource("track-2", {
			expires_at: "2026-01-01T01:00:00Z",
			path: "/api/v1/s/share-token/stream/session-2/two.mp3",
		});

		expect(useMusicPlayerStore.getState().queue).toEqual([
			expect.objectContaining({
				id: "track-1",
				expiresAt: "2026-01-01T00:30:00Z",
				path: "/api/v1/s/share-token/stream/session-1/one.mp3",
			}),
			expect.objectContaining({
				id: "track-2",
				expiresAt: "2026-01-01T01:00:00Z",
				path: "/api/v1/s/share-token/stream/session-2/two.mp3",
			}),
		]);
	});

	it("merges parsed metadata into only the matching queued track", async () => {
		const { useMusicPlayerStore } = await loadMusicPlayerStore();

		useMusicPlayerStore.getState().playTracks(
			[
				{
					id: "track-1",
					metadata: { artist: "Artist One", title: "Track One" },
					mimeType: "audio/mpeg",
					name: "one.mp3",
					path: "/files/1/download",
				},
				{
					id: "track-2",
					metadata: { title: "Track Two" },
					mimeType: "audio/mpeg",
					name: "two.mp3",
					path: "/files/2/download",
				},
			],
			"track-1",
		);

		useMusicPlayerStore.getState().updateTrackMetadata("track-1", {
			album: "Album One",
			artworkUrl: "data:image/jpeg;base64,cover",
			title: "Parsed Title",
		});
		useMusicPlayerStore.getState().updateTrackMetadata("missing-track", {
			title: "Ignored",
		});

		expect(useMusicPlayerStore.getState().queue).toEqual([
			expect.objectContaining({
				id: "track-1",
				metadata: {
					album: "Album One",
					artist: "Artist One",
					artworkUrl: "data:image/jpeg;base64,cover",
					title: "Parsed Title",
				},
			}),
			expect.objectContaining({
				id: "track-2",
				metadata: { title: "Track Two" },
			}),
		]);
	});

	it("clears queue, playback state, errors, panel state, and request counters", async () => {
		const { useMusicPlayerStore } = await loadMusicPlayerStore();

		useMusicPlayerStore.getState().playTrack({
			id: "track-1",
			mimeType: "audio/mpeg",
			name: "track.mp3",
			path: "/files/7/download",
		});
		useMusicPlayerStore.getState().openPanel();
		useMusicPlayerStore.getState().setError("load failed");
		useMusicPlayerStore.getState().setPlaying(true);
		useMusicPlayerStore.getState().clear();

		expect(useMusicPlayerStore.getState()).toMatchObject({
			activeTrackId: null,
			error: null,
			isPanelOpen: false,
			isPlaying: false,
			playRequested: false,
			playRequestVersion: 0,
			queue: [],
		});
	});
});
