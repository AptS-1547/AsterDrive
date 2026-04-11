import { act, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { PreviewAppsConfigEditor } from "@/components/admin/PreviewAppsConfigEditor";
import { PREVIEW_APP_ICON_URLS } from "@/components/common/previewAppIconUrls";

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		i18n: { language: "zh-CN" },
		t: (key: string, values?: Record<string, number | string>) => {
			if (!values) {
				return key;
			}

			return Object.entries(values).reduce(
				(result, [name, value]) =>
					result.replaceAll(`{{${name}}}`, String(value)),
				key,
			);
		},
	}),
}));

function createPreviewAppsConfig() {
	return JSON.stringify(
		{
			version: 1,
			apps: [
				{
					key: "custom.viewer",
					provider: "url_template",
					icon: "https://viewer.example.com/icon.svg",
					enabled: true,
					labels: {
						en: "Viewer",
						zh: "外部查看器",
					},
					config: {
						mode: "iframe",
						url_template:
							"https://viewer.example.com/embed?src={{file_preview_url}}",
						allowed_origins: ["https://viewer.example.com"],
					},
				},
			],
			rules: [],
		},
		null,
		2,
	);
}

function createPreviewAppsConfigWithRule() {
	return JSON.stringify(
		{
			version: 1,
			apps: [
				{
					key: "custom.viewer",
					provider: "url_template",
					icon: "https://viewer.example.com/icon.svg",
					enabled: true,
					labels: {
						en: "Viewer",
						zh: "外部查看器",
					},
					config: {
						mode: "iframe",
						url_template:
							"https://viewer.example.com/embed?src={{file_preview_url}}",
						allowed_origins: ["https://viewer.example.com"],
					},
				},
			],
			rules: [
				{
					apps: ["custom.viewer"],
					default_app: "custom.viewer",
					matches: {
						categories: ["spreadsheet"],
						extensions: [".xlsx"],
						mime_types: [
							"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
						],
						mime_prefixes: [],
					},
				},
			],
		},
		null,
		2,
	);
}

function createPreviewAppsConfigWithDefaultIcon() {
	return JSON.stringify(
		{
			version: 1,
			apps: [
				{
					key: "builtin.image",
					provider: "builtin",
					icon: PREVIEW_APP_ICON_URLS.image,
					enabled: true,
					labels: {
						zh: "图片预览",
					},
				},
			],
			rules: [],
		},
		null,
		2,
	);
}

function StatefulPreviewAppsEditor({
	initialValue = createPreviewAppsConfig(),
}: {
	initialValue?: string;
}) {
	const [value, setValue] = useState(initialValue);
	return <PreviewAppsConfigEditor value={value} onChange={setValue} />;
}

describe("PreviewAppsConfigEditor", () => {
	it("keeps the focused input active while typing in an expanded app row", () => {
		render(<StatefulPreviewAppsEditor />);

		fireEvent.click(
			screen.getByRole("button", { name: "preview_apps_expand" }),
		);

		const keyInput = screen.getByDisplayValue("custom.viewer");
		keyInput.focus();
		expect(keyInput).toHaveFocus();

		fireEvent.change(keyInput, {
			target: { value: "custom.viewer.updated" },
		});

		expect(screen.getByDisplayValue("custom.viewer.updated")).toHaveFocus();
	});

	it("opens the URL template magic variables dialog", () => {
		render(<StatefulPreviewAppsEditor />);

		fireEvent.click(
			screen.getByRole("button", { name: "preview_apps_expand" }),
		);
		fireEvent.click(
			screen.getByRole("button", {
				name: "preview_apps_url_template_variables_link",
			}),
		);

		expect(
			screen.getByText("preview_apps_url_template_variables_dialog_desc"),
		).toBeInTheDocument();
		expect(screen.getByText("{{download_path}}")).toBeInTheDocument();
		expect(screen.getByText("{{file_preview_url}}")).toBeInTheDocument();
	});

	it("shows an empty icon input when the configured icon matches the default", () => {
		render(
			<StatefulPreviewAppsEditor
				initialValue={createPreviewAppsConfigWithDefaultIcon()}
			/>,
		);

		fireEvent.click(
			screen.getByRole("button", { name: "preview_apps_expand" }),
		);

		const iconField = screen
			.getAllByText("preview_apps_icon_label")[1]
			?.parentElement?.querySelector("input");
		expect(iconField).not.toBeNull();
		expect(iconField).toHaveValue("");
	});

	it("keeps expanded content mounted until the collapse animation finishes", () => {
		vi.useFakeTimers();

		render(<StatefulPreviewAppsEditor />);

		fireEvent.click(
			screen.getByRole("button", { name: "preview_apps_expand" }),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "preview_apps_collapse" }),
		);

		expect(screen.getByDisplayValue("custom.viewer")).toBeInTheDocument();

		act(() => {
			vi.runAllTimers();
		});

		expect(screen.queryByDisplayValue("custom.viewer")).not.toBeInTheDocument();
	});

	it("shows readable rule summaries before expanding a rule", () => {
		render(
			<StatefulPreviewAppsEditor
				initialValue={createPreviewAppsConfigWithRule()}
			/>,
		);

		expect(screen.getByText("spreadsheet")).toBeInTheDocument();
		expect(screen.getByText(".xlsx")).toBeInTheDocument();
		expect(screen.getAllByText("外部查看器").length).toBeGreaterThan(0);
		expect(
			screen.getByText("preview_apps_rule_default_badge"),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("combobox", {
				name: "preview_apps_rule_default_label",
			}),
		).not.toBeInTheDocument();
	});
});
