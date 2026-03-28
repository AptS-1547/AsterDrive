import { describe, expect, it } from "vitest";
import {
	detectFilePreviewProfile,
	getAvailableOpenWithOptions,
	getDefaultOpenWith,
	getEditorLanguage,
	getFileExtension,
	getFileTypeInfo,
	isEditableTextFile,
} from "@/components/files/preview/file-capabilities";

describe("file preview capabilities", () => {
	it("detects file extensions and editor languages", () => {
		expect(
			getFileExtension({ name: "README.MD", mime_type: "text/markdown" }),
		).toBe("md");
		expect(
			getEditorLanguage({ name: "Dockerfile", mime_type: "text/plain" }),
		).toBe("dockerfile");
		expect(getEditorLanguage({ name: ".env", mime_type: "text/plain" })).toBe(
			"plaintext",
		);
		expect(
			getEditorLanguage({ name: "script.tsx", mime_type: "text/typescript" }),
		).toBe("typescript");
		// Newly added extensions
		expect(
			getEditorLanguage({ name: "App.vue", mime_type: "text/plain" }),
		).toBe("html");
		expect(
			getEditorLanguage({ name: "main.dart", mime_type: "text/plain" }),
		).toBe("dart");
		expect(
			getEditorLanguage({ name: "buf.proto", mime_type: "text/plain" }),
		).toBe("protobuf");
		expect(
			getEditorLanguage({ name: "schema.graphql", mime_type: "text/plain" }),
		).toBe("graphql");
		expect(
			getEditorLanguage({ name: "main.tf", mime_type: "text/plain" }),
		).toBe("hcl");
		expect(
			getEditorLanguage({ name: "build.sbt", mime_type: "text/plain" }),
		).toBe("scala");
		expect(getEditorLanguage({ name: "app.ex", mime_type: "text/plain" })).toBe(
			"elixir",
		);
		expect(
			getEditorLanguage({ name: "build.gradle", mime_type: "text/plain" }),
		).toBe("java");
		expect(
			getEditorLanguage({ name: "intro.tex", mime_type: "text/plain" }),
		).toBe("plaintext");
		expect(
			getEditorLanguage({ name: "deploy.ps1", mime_type: "text/plain" }),
		).toBe("powershell");
		// Special filenames
		expect(
			getEditorLanguage({ name: ".dockerignore", mime_type: "text/plain" }),
		).toBe("plaintext");
		expect(
			getEditorLanguage({ name: "Jenkinsfile", mime_type: "text/plain" }),
		).toBe("groovy");
		expect(
			getEditorLanguage({ name: "Gemfile", mime_type: "text/plain" }),
		).toBe("ruby");
	});

	it("maps mime types and extensions to file categories", () => {
		expect(
			getFileTypeInfo({ name: "manual.pdf", mime_type: "application/pdf" }),
		).toMatchObject({
			category: "pdf",
			icon: "FileText",
		});
		expect(
			getFileTypeInfo({
				name: "table.csv",
				mime_type: "application/octet-stream",
			}),
		).toMatchObject({
			category: "csv",
			icon: "Table",
		});
		expect(
			getFileTypeInfo({ name: "photo.svg", mime_type: "image/svg+xml" }),
		).toMatchObject({
			category: "image",
		});
		expect(
			getFileTypeInfo({ name: "notes.txt", mime_type: "text/xml" }),
		).toMatchObject({
			category: "xml",
			icon: "BracketsCurly",
		});
		expect(
			getFileTypeInfo({
				name: "archive.bin",
				mime_type: "application/octet-stream",
			}),
		).toMatchObject({
			category: "unknown",
			icon: "File",
		});
	});

	it("derives preview profiles and open-with options", () => {
		const markdown = { name: "notes.md", mime_type: "text/markdown" };
		const json = { name: "data.json", mime_type: "application/json" };
		const image = { name: "photo.png", mime_type: "image/png" };
		const tsv = {
			name: "report.tsv",
			mime_type: "text/tab-separated-values",
		};
		const shell = { name: "deploy", mime_type: "application/x-sh" };
		const unknown = {
			name: "archive.bin",
			mime_type: "application/octet-stream",
		};

		expect(detectFilePreviewProfile(markdown)).toMatchObject({
			category: "markdown",
			isBlobPreview: false,
			isTextBased: true,
			isEditableText: true,
			defaultMode: "markdown",
		});
		expect(detectFilePreviewProfile(json)).toMatchObject({
			category: "json",
			defaultMode: "formatted",
		});
		expect(detectFilePreviewProfile(image)).toMatchObject({
			category: "image",
			isBlobPreview: true,
			defaultMode: "image",
		});
		expect(detectFilePreviewProfile(tsv)).toMatchObject({
			category: "tsv",
			defaultMode: "table",
			isEditableText: true,
		});
		expect(detectFilePreviewProfile(shell)).toMatchObject({
			category: "text",
			defaultMode: "code",
		});
		expect(detectFilePreviewProfile(unknown)).toMatchObject({
			category: "unknown",
			defaultMode: null,
			isEditableText: true,
			options: [{ mode: "code", labelKey: "open_with_try_text" }],
		});

		expect(getAvailableOpenWithOptions(json)).toEqual([
			expect.objectContaining({ mode: "formatted" }),
			expect.objectContaining({ mode: "code" }),
		]);
		expect(getDefaultOpenWith(json)).toBe("formatted");
		expect(getDefaultOpenWith(tsv)).toBe("table");
		expect(isEditableTextFile(markdown)).toBe(true);
		expect(isEditableTextFile(image)).toBe(false);
		expect(isEditableTextFile(shell)).toBe(true);
	});

	it("recognizes newly added text extensions", () => {
		const vue = { name: "App.vue", mime_type: "application/octet-stream" };
		const dart = { name: "main.dart", mime_type: "application/octet-stream" };
		const proto = { name: "api.proto", mime_type: "application/octet-stream" };
		const tf = { name: "main.tf", mime_type: "application/octet-stream" };

		for (const file of [vue, dart, proto, tf]) {
			expect(isEditableTextFile(file)).toBe(true);
			expect(getDefaultOpenWith(file)).toBe("code");
		}
	});

	it("provides text fallback for unknown files but not for known binary types", () => {
		const unknown = {
			name: "mystery.xyz",
			mime_type: "application/octet-stream",
		};
		const archive = {
			name: "data.zip",
			mime_type: "application/zip",
		};

		expect(detectFilePreviewProfile(unknown).isEditableText).toBe(true);
		expect(detectFilePreviewProfile(unknown).options).toEqual([
			expect.objectContaining({ mode: "code", labelKey: "open_with_try_text" }),
		]);
		expect(detectFilePreviewProfile(unknown).defaultMode).toBeNull();

		expect(detectFilePreviewProfile(archive).isEditableText).toBe(false);
		expect(detectFilePreviewProfile(archive).options).toEqual([]);
	});
});
