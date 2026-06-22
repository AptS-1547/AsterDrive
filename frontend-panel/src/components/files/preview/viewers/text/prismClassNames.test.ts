import { describe, expect, it } from "vitest";
import {
	scopePrismClassName,
	withScopedPrismClassName,
} from "@/components/files/preview/viewers/text/prismClassNames";

describe("prismClassNames", () => {
	it("prefixes Prism token class names to avoid global utility collisions", () => {
		expect(scopePrismClassName("token table class-name")).toBe(
			"prism-token prism-table prism-class-name",
		);
	});

	it("preserves existing prism-prefixed names", () => {
		expect(scopePrismClassName("prism-token prism-table")).toBe(
			"prism-token prism-table",
		);
	});

	it("scopes props objects without changing other fields", () => {
		expect(
			withScopedPrismClassName({
				children: "server",
				className: "token table class-name",
				style: { color: "red" },
			}),
		).toEqual({
			children: "server",
			className: "prism-token prism-table prism-class-name",
			style: { color: "red" },
		});
	});
});
