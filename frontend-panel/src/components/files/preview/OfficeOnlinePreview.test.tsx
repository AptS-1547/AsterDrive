import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { OfficeOnlinePreview } from "@/components/files/preview/OfficeOnlinePreview";

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, options?: Record<string, unknown>) => {
			if (options?.provider) {
				return `${key}:${options.provider}`;
			}
			return key;
		},
	}),
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		onClick,
		className,
	}: {
		children: React.ReactNode;
		onClick?: () => void;
		className?: string;
	}) => (
		<button type="button" onClick={onClick} className={className}>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{name}</span>,
}));

vi.mock("@/components/files/preview/PreviewLoadingState", () => ({
	PreviewLoadingState: ({ text }: { text: string }) => <div>{text}</div>,
}));

vi.mock("@/components/files/preview/PreviewError", () => ({
	PreviewError: ({ onRetry }: { onRetry?: () => void }) => (
		<div>
			<div>preview-error</div>
			{onRetry ? (
				<button type="button" onClick={onRetry}>
					retry-preview
				</button>
			) : null}
		</div>
	),
}));

describe("OfficeOnlinePreview", () => {
	afterEach(() => {
		vi.useRealTimers();
		vi.restoreAllMocks();
	});

	it("loads the Microsoft viewer by default for docx files", async () => {
		const createPreviewLink = vi.fn(async () => ({
			expires_at: "2026-04-08T12:00:00Z",
			max_uses: 5,
			path: "https://files.example.com/pv/token/report.docx",
		}));

		render(
			<OfficeOnlinePreview
				file={{
					name: "report.docx",
					mime_type:
						"application/vnd.openxmlformats-officedocument.wordprocessingml.document",
				}}
				downloadPath="/files/7/download"
				createPreviewLink={createPreviewLink}
			/>,
		);

		await waitFor(() => {
			expect(createPreviewLink).toHaveBeenCalled();
		});

		const iframe = await screen.findByTitle("report.docx");
		expect(iframe.getAttribute("src")).toContain("view.officeapps.live.com");
	});

	it("switches providers and rebuilds the viewer url", async () => {
		const createPreviewLink = vi.fn(async () => ({
			expires_at: "2026-04-08T12:00:00Z",
			max_uses: 5,
			path: "https://files.example.com/pv/token/report.docx",
		}));

		render(
			<OfficeOnlinePreview
				file={{
					name: "report.docx",
					mime_type:
						"application/vnd.openxmlformats-officedocument.wordprocessingml.document",
				}}
				downloadPath="/files/7/download"
				createPreviewLink={createPreviewLink}
			/>,
		);

		await waitFor(() => {
			expect(createPreviewLink).toHaveBeenCalled();
		});
		const initialCalls = createPreviewLink.mock.calls.length;

		fireEvent.click(
			screen.getByRole("button", { name: /office_provider_google/ }),
		);

		await waitFor(() => {
			expect(createPreviewLink.mock.calls.length).toBeGreaterThan(initialCalls);
		});
		const iframe = await screen.findByTitle("report.docx");
		expect(iframe.getAttribute("src")).toContain("docs.google.com");
	});

	it("shows a fallback when the preview link cannot be prepared and retries", async () => {
		const createPreviewLink = vi.fn(async () => {
			throw new Error("preview link failed");
		});

		render(
			<OfficeOnlinePreview
				file={{
					name: "report.docx",
					mime_type:
						"application/vnd.openxmlformats-officedocument.wordprocessingml.document",
				}}
				downloadPath="/files/7/download"
				createPreviewLink={createPreviewLink}
			/>,
		);

		await waitFor(() => {
			expect(screen.getByText("preview-error")).toBeInTheDocument();
		});
		expect(
			screen.getByText("office_preview_error_desc:office_provider_microsoft"),
		).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "retry-preview" }));

		await waitFor(() => {
			expect(createPreviewLink.mock.calls.length).toBeGreaterThan(1);
		});
	});

	it("opens the direct download path from the fallback action", async () => {
		const createPreviewLink = vi.fn(async () => ({
			expires_at: "2026-04-08T12:00:00Z",
			max_uses: 5,
			path: "https://files.example.com/pv/token/report.docx",
		}));
		const openSpy = vi.spyOn(window, "open").mockReturnValue(null);

		render(
			<OfficeOnlinePreview
				file={{
					name: "report.docx",
					mime_type:
						"application/vnd.openxmlformats-officedocument.wordprocessingml.document",
				}}
				downloadPath="/files/7/download"
				createPreviewLink={createPreviewLink}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: /download/i }));

		expect(openSpy).toHaveBeenCalledWith(
			"/files/7/download",
			"_blank",
			"noopener,noreferrer",
		);
	});

	it("shows an error state immediately for localhost preview links", async () => {
		const createPreviewLink = vi.fn(async () => ({
			expires_at: "2026-04-08T12:00:00Z",
			max_uses: 5,
			path: "/pv/token/report.docx",
		}));

		render(
			<OfficeOnlinePreview
				file={{
					name: "report.docx",
					mime_type:
						"application/vnd.openxmlformats-officedocument.wordprocessingml.document",
				}}
				downloadPath="/files/7/download"
				createPreviewLink={createPreviewLink}
			/>,
		);

		await waitFor(() => {
			expect(
				screen.getByText(
					"office_preview_public_url_desc:office_provider_microsoft",
				),
			).toBeInTheDocument();
		});
		await new Promise((resolve) => window.setTimeout(resolve, 20));
		expect(createPreviewLink).toHaveBeenCalledTimes(1);
		expect(screen.queryByTitle("report.docx")).not.toBeInTheDocument();
	});

	it("shows an error state immediately for public http preview links", async () => {
		const createPreviewLink = vi.fn(async () => ({
			expires_at: "2026-04-08T12:00:00Z",
			max_uses: 5,
			path: "http://files.example.com/pv/token/report.docx",
		}));

		render(
			<OfficeOnlinePreview
				file={{
					name: "report.docx",
					mime_type:
						"application/vnd.openxmlformats-officedocument.wordprocessingml.document",
				}}
				downloadPath="/files/7/download"
				createPreviewLink={createPreviewLink}
			/>,
		);

		await waitFor(() => {
			expect(
				screen.getByText(
					"office_preview_https_required_desc:office_provider_microsoft",
				),
			).toBeInTheDocument();
		});
		await new Promise((resolve) => window.setTimeout(resolve, 20));
		expect(createPreviewLink).toHaveBeenCalledTimes(1);
		expect(screen.queryByTitle("report.docx")).not.toBeInTheDocument();
	});
});
