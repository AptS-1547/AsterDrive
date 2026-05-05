import { describe, expect, it } from "vitest";
import {
	IME_COMPOSITION_END_GRACE_MS,
	isImeComposingKeyEvent,
	shouldIgnoreKeyboardTarget,
} from "@/lib/keyboard";

describe("shouldIgnoreKeyboardTarget", () => {
	it("ignores editable form controls and contenteditable targets", () => {
		const input = document.createElement("input");
		const textarea = document.createElement("textarea");
		const select = document.createElement("select");
		const editable = document.createElement("div");
		Object.defineProperty(editable, "isContentEditable", {
			configurable: true,
			value: true,
		});

		expect(shouldIgnoreKeyboardTarget(input)).toBe(true);
		expect(shouldIgnoreKeyboardTarget(textarea)).toBe(true);
		expect(shouldIgnoreKeyboardTarget(select)).toBe(true);
		expect(shouldIgnoreKeyboardTarget(editable)).toBe(true);
		expect(shouldIgnoreKeyboardTarget(document.body)).toBeFalsy();
		expect(shouldIgnoreKeyboardTarget(null)).toBeFalsy();
	});
});

describe("isImeComposingKeyEvent", () => {
	it("detects browser IME composition signals", () => {
		expect(
			isImeComposingKeyEvent({
				key: "Enter",
				nativeEvent: { isComposing: true },
			}),
		).toBe(true);
		expect(
			isImeComposingKeyEvent({
				key: "Enter",
				nativeEvent: { keyCode: 229 },
			}),
		).toBe(true);
		expect(isImeComposingKeyEvent({ key: "Process" })).toBe(true);
	});

	it("keeps ignoring keys briefly after compositionend for Safari ordering", () => {
		expect(
			isImeComposingKeyEvent(
				{ key: "Enter" },
				{
					lastCompositionEndAt: 1000,
					now: 1000 + IME_COMPOSITION_END_GRACE_MS - 1,
				},
			),
		).toBe(true);
		expect(
			isImeComposingKeyEvent(
				{ key: "Enter" },
				{
					lastCompositionEndAt: 1000,
					now: 1000 + IME_COMPOSITION_END_GRACE_MS,
				},
			),
		).toBe(false);
	});

	it("allows normal keyboard events", () => {
		expect(isImeComposingKeyEvent({ key: "Enter" })).toBe(false);
		expect(isImeComposingKeyEvent({ key: "a", keyCode: 65 })).toBe(false);
	});
});
