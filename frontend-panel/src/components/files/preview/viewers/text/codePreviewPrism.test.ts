import { describe, expect, it } from "vitest";
import {
	createEditorPalette,
	ensurePrismLanguage,
	getPrismLanguageConfig,
	hasPrismGrammar,
	normalizePrismLanguage,
	Prism,
} from "@/components/files/preview/viewers/text/codePreviewPrism";

describe("codePreviewPrism", () => {
	it("maps aliases and unknown languages to Prism grammars", () => {
		expect(normalizePrismLanguage("typescript")).toBe("typescript");
		expect(normalizePrismLanguage("shell")).toBe("bash");
		expect(normalizePrismLanguage("dockerfile")).toBe("docker");
		expect(normalizePrismLanguage("unknown-language")).toBe("text");

		expect(getPrismLanguageConfig("php")).toEqual({
			components: ["php"],
			grammar: "php",
		});
		expect(getPrismLanguageConfig("unknown-language")).toEqual({
			components: [],
			grammar: "text",
		});
	});

	it("returns editor palettes for light and dark themes", () => {
		expect(createEditorPalette("vs-dark")).toMatchObject({
			background: "#1e1e1e",
			border: "#2a2a2a",
			caret: "#ffffff",
		});
		expect(createEditorPalette("vs-light")).toMatchObject({
			background: "#ffffff",
			border: "#d0d7de",
			caret: "#1f2328",
		});
	});

	it("detects loaded Prism grammars and always accepts plain text", () => {
		expect(hasPrismGrammar("text")).toBe(true);
		expect(hasPrismGrammar("definitely-missing" as never)).toBe(false);
	});

	it("loads Prism components with dependencies and caches repeated requests", async () => {
		await ensurePrismLanguage(getPrismLanguageConfig("typescript"));

		expect(hasPrismGrammar("clike")).toBe(true);
		expect(hasPrismGrammar("javascript")).toBe(true);
		expect(hasPrismGrammar("typescript")).toBe(true);

		const javascriptGrammar = Prism.languages.javascript;
		await ensurePrismLanguage(getPrismLanguageConfig("typescript"));

		expect(Prism.languages.javascript).toBe(javascriptGrammar);
	});

	it("resolves immediately for fallback languages with no components", async () => {
		await expect(
			ensurePrismLanguage(getPrismLanguageConfig("plaintext")),
		).resolves.toBeUndefined();
	});
});
