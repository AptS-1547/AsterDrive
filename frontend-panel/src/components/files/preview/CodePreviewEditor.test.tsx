import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { CodePreviewEditor } from "@/components/files/preview/CodePreviewEditor";

describe("CodePreviewEditor", () => {
	it("disables textarea soft wrapping when wordWrap is off", () => {
		render(
			<CodePreviewEditor
				language="plaintext"
				theme="vs"
				value={"const url = 'https://example.com/really/long/path';"}
				options={{
					readOnly: false,
					wordWrap: "off",
				}}
			/>,
		);

		expect(screen.getByLabelText("Code editor")).toHaveAttribute("wrap", "off");
	});

	it("keeps preformatted lines unwrapped in read-only mode", () => {
		const { container } = render(
			<CodePreviewEditor
				language="plaintext"
				theme="vs"
				value={"const url = 'https://example.com/really/long/path';"}
				options={{
					readOnly: true,
					wordWrap: "off",
				}}
			/>,
		);

		expect(container.querySelector("pre")).toHaveStyle({ whiteSpace: "pre" });
	});
});
