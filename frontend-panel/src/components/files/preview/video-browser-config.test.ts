import { beforeEach, describe, expect, it } from "vitest";
import {
	parseVideoBrowserConfig,
	resolveVideoBrowserTarget,
} from "@/components/files/preview/video-browser-config";

describe("video browser config", () => {
	beforeEach(() => {
		window.history.replaceState({}, "", "/files");
	});

	it("parses config with defaults", () => {
		expect(
			parseVideoBrowserConfig({
				VITE_VIDEO_BROWSER_URL_TEMPLATE: "/watch?file={{fileId}}",
			}),
		).toEqual({
			label: "Custom Video Browser",
			mode: "iframe",
			urlTemplate: "/watch?file={{fileId}}",
			allowedOrigins: [],
		});
	});

	it("resolves same-origin templates with encoded values", () => {
		const origin = window.location.origin;
		const config = parseVideoBrowserConfig({
			VITE_VIDEO_BROWSER_LABEL: "Jellyfin",
			VITE_VIDEO_BROWSER_URL_TEMPLATE:
				"/watch?file={{fileId}}&name={{fileName}}&src={{downloadUrl}}",
		});

		const target = resolveVideoBrowserTarget(
			{
				id: 7,
				name: "clip 1.mp4",
				mime_type: "video/mp4",
				size: 2048,
			},
			"/api/v1/files/7/download",
			config,
		);

		expect(target).toEqual({
			label: "Jellyfin",
			mode: "iframe",
			url: `${origin}/watch?file=7&name=clip%201.mp4&src=${encodeURIComponent(`${origin}/api/v1/files/7/download`)}`,
		});
	});

	it("rejects cross-origin targets that are not explicitly allowed", () => {
		const config = parseVideoBrowserConfig({
			VITE_VIDEO_BROWSER_LABEL: "Jellyfin",
			VITE_VIDEO_BROWSER_URL_TEMPLATE:
				"https://videos.example.com/watch?file={{fileId}}",
		});

		expect(
			resolveVideoBrowserTarget(
				{
					id: 7,
					name: "clip.mp4",
					mime_type: "video/mp4",
				},
				"/api/v1/files/7/download",
				config,
			),
		).toBeNull();
	});

	it("allows whitelisted cross-origin targets and supports new-tab mode", () => {
		const config = parseVideoBrowserConfig({
			VITE_VIDEO_BROWSER_LABEL: "Jellyfin",
			VITE_VIDEO_BROWSER_MODE: "new_tab",
			VITE_VIDEO_BROWSER_URL_TEMPLATE:
				"https://videos.example.com/watch?file={{fileId}}",
			VITE_VIDEO_BROWSER_ALLOWED_ORIGINS: "https://videos.example.com",
		});

		expect(
			resolveVideoBrowserTarget(
				{
					id: 7,
					name: "clip.mp4",
					mime_type: "video/mp4",
				},
				"/api/v1/files/7/download",
				config,
			),
		).toEqual({
			label: "Jellyfin",
			mode: "new_tab",
			url: "https://videos.example.com/watch?file=7",
		});
	});
});
