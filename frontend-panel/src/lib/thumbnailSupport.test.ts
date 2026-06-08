import { describe, expect, it } from "vitest";
import {
	getThumbnailExtension,
	imagePreviewExtensionCandidatesFromMime,
	supportsGeneratedThumbnailFile,
	supportsImagePreviewFile,
	supportsThumbnailExtension,
} from "@/lib/thumbnailSupport";

describe("thumbnailSupport", () => {
	it("normalizes thumbnail file extensions", () => {
		expect(getThumbnailExtension(" Photo.JPG ")).toBe("jpg");
		expect(getThumbnailExtension("archive.tar.gz")).toBe("gz");
		expect(getThumbnailExtension(".gitignore")).toBe("");
		expect(getThumbnailExtension("no-extension")).toBe("");
		expect(getThumbnailExtension("trailing.")).toBe("");
	});

	it("checks configured thumbnail support case-insensitively", () => {
		expect(
			supportsThumbnailExtension("cover.MP3", [" .jpg ", ".mp3", "png"]),
		).toBe(true);
		expect(supportsThumbnailExtension("cover.mp3", [])).toBe(false);
		expect(supportsThumbnailExtension("cover.mp3", undefined)).toBe(false);
		expect(supportsThumbnailExtension("cover", ["mp3"])).toBe(false);
		expect(supportsThumbnailExtension("cover.flac", ["mp3"])).toBe(false);
	});

	it("checks generated thumbnail support across media groups", () => {
		const config = {
			version: 1,
			image_preview: {
				enabled: true,
				extensions: ["png"],
			},
			image_thumbnail: {
				enabled: true,
				extensions: ["jpg"],
			},
			audio_thumbnail: {
				enabled: false,
				extensions: ["mp3"],
			},
			video_thumbnail: {
				enabled: true,
				extensions: ["mp4", "mov"],
			},
		};

		expect(supportsGeneratedThumbnailFile("clip.MP4", config)).toBe(true);
		expect(supportsGeneratedThumbnailFile("track.mp3", config)).toBe(false);
		expect(supportsGeneratedThumbnailFile("photo.png", config)).toBe(false);
		expect(supportsGeneratedThumbnailFile("photo.jpg", config)).toBe(true);
		expect(supportsGeneratedThumbnailFile("clip.mp4", null)).toBe(false);
	});

	it("matches image preview support by extension or MIME subtype", () => {
		expect(supportsImagePreviewFile("photo.HEIC", "", ["heic"])).toBe(true);
		expect(supportsImagePreviewFile("upload", "image/heic", ["heic"])).toBe(
			true,
		);
		expect(supportsImagePreviewFile("upload", "image/tiff", ["tif"])).toBe(
			true,
		);
		expect(supportsImagePreviewFile("upload", "image/jpeg", ["jpg"])).toBe(
			true,
		);
		expect(supportsImagePreviewFile("upload", "image/heic", ["png"])).toBe(
			false,
		);
		expect(imagePreviewExtensionCandidatesFromMime("image/svg+xml")).toEqual([
			"svg",
		]);
		expect(imagePreviewExtensionCandidatesFromMime("application/json")).toEqual(
			[],
		);
	});
});
