import { beforeEach, describe, expect, it } from "vitest";
import {
	applyBranding,
	DEFAULT_BRANDING,
	resolveBranding,
} from "@/lib/branding";

describe("branding helpers", () => {
	beforeEach(() => {
		document.title = "";
		for (const selector of [
			'meta[name="description"]',
			'link[rel="icon"]',
			'link[rel="apple-touch-icon"]',
		]) {
			document.head.querySelector(selector)?.remove();
		}
	});

	it("falls back to defaults for blank fields and invalid favicon URLs", () => {
		expect(
			resolveBranding({
				title: "   ",
				description: "",
				favicon_url: "javascript:alert(1)",
			}),
		).toEqual(DEFAULT_BRANDING);
	});

	it("normalizes safe favicon URLs against the current origin", () => {
		expect(
			resolveBranding({
				title: "My Drive",
				description: "Team storage",
				favicon_url: "/assets/brand/icon.png?v=1",
			}),
		).toEqual({
			title: "My Drive",
			description: "Team storage",
			faviconUrl: "/assets/brand/icon.png?v=1",
		});
	});

	it("rejects non-root relative favicon paths", () => {
		expect(
			resolveBranding({
				title: "My Drive",
				description: "Team storage",
				favicon_url: "assets/brand/icon.png",
			}),
		).toEqual({
			title: "My Drive",
			description: "Team storage",
			faviconUrl: "/favicon.svg",
		});
	});

	it("updates title, description, and both icon links", () => {
		applyBranding({
			title: "Nebula Drive",
			description: "Private cloud for the squad",
			faviconUrl: "https://cdn.example.com/brand/favicon.png",
		});

		expect(document.title).toBe("Nebula Drive");
		expect(
			document.head.querySelector('meta[name="description"]'),
		).toHaveAttribute("content", "Private cloud for the squad");
		expect(document.head.querySelector('link[rel="icon"]')).toHaveAttribute(
			"href",
			"https://cdn.example.com/brand/favicon.png",
		);
		expect(
			document.head.querySelector('link[rel="apple-touch-icon"]'),
		).toHaveAttribute("href", "https://cdn.example.com/brand/favicon.png");
		expect(document.head.querySelector('link[rel="icon"]')).not.toHaveAttribute(
			"type",
		);
	});

	it("preserves non-ascii branding text in the document head", () => {
		applyBranding({
			title: "猫猫云盘",
			description: "团队私有云存储",
			faviconUrl: "https://cdn.example.com/brand/favicon.png",
		});

		expect(document.title).toBe("猫猫云盘");
		expect(
			document.head.querySelector('meta[name="description"]'),
		).toHaveAttribute("content", "团队私有云存储");
	});
});
