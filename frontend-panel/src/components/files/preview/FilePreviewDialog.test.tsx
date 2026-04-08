import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FilePreviewDialog } from "@/components/files/preview/FilePreviewDialog";

const mockState = vi.hoisted(() => ({
	downloadPath: vi.fn((fileId: number) => `/files/${fileId}/download`),
	getStoredPreference: vi.fn(),
	profile: {
		category: "markdown",
		defaultMode: "code",
		isBlobPreview: false,
		isEditableText: true,
		isTextBased: true,
		options: [
			{ icon: "TextT", labelKey: "mode_code", mode: "code" },
			{ icon: "MarkdownLogo", labelKey: "mode_markdown", mode: "markdown" },
		],
	},
	setStoredPreference: vi.fn(),
	videoBrowserOption: null as {
		icon: string;
		label?: string;
		labelKey: string;
		mode: string;
	} | null,
	videoBrowserTarget: null as {
		label: string;
		mode: "iframe" | "new_tab";
		url: string;
	} | null,
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, opts?: Record<string, unknown>) => {
			if (key === "files:open_modes_count") {
				return `modes:${opts?.count}`;
			}
			return key;
		},
	}),
}));

vi.mock("@/components/files/FileTypeIcon", () => ({
	FileTypeIcon: ({
		mimeType,
		fileName,
	}: {
		mimeType: string;
		fileName: string;
	}) => <span>{`${mimeType}:${fileName}`}</span>,
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

vi.mock("@/components/ui/dialog", () => ({
	Dialog: ({ children }: { children: React.ReactNode }) => (
		<div data-testid="dialog">{children}</div>
	),
	DialogContent: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => (
		<div data-testid="dialog-content" className={className}>
			{children}
		</div>
	),
	DialogHeader: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => <div className={className}>{children}</div>,
	DialogTitle: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => <h2 className={className}>{children}</h2>,
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{name}</span>,
}));

vi.mock("@/components/ui/scroll-area", () => ({
	ScrollArea: ({
		children,
		className,
	}: {
		children: React.ReactNode;
		className?: string;
	}) => <div className={className}>{children}</div>,
}));

vi.mock("@/lib/format", () => ({
	formatBytes: (value: number) => `bytes:${value}`,
}));

vi.mock("@/services/fileService", () => ({
	fileService: {
		downloadPath: (...args: unknown[]) => mockState.downloadPath(...args),
	},
}));

vi.mock("@/components/files/preview/BlobMediaPreview", () => ({
	BlobMediaPreview: ({ mode, path }: { mode: string; path: string }) => (
		<div>{`blob:${mode}:${path}`}</div>
	),
}));

vi.mock("@/components/files/preview/CustomVideoBrowserPreview", () => ({
	CustomVideoBrowserPreview: ({
		target,
	}: {
		target: { url: string } | null;
	}) => <div>{`video-browser:${target?.url ?? "null"}`}</div>,
}));

vi.mock("@/components/files/preview/file-capabilities", () => ({
	detectFilePreviewProfile: () => mockState.profile,
}));

vi.mock("@/components/files/preview/open-with-preferences", () => ({
	getStoredOpenWithPreference: (...args: unknown[]) =>
		mockState.getStoredPreference(...args),
	setStoredOpenWithPreference: (...args: unknown[]) =>
		mockState.setStoredPreference(...args),
}));

vi.mock("@/components/files/preview/video-browser-config", () => ({
	getVideoBrowserOpenWithOption: () => mockState.videoBrowserOption,
	resolveVideoBrowserTarget: () => mockState.videoBrowserTarget,
}));

vi.mock("@/components/files/preview/PreviewModeSwitch", () => ({
	PreviewModeSwitch: ({
		options,
		value,
		onChange,
	}: {
		options: Array<{ labelKey: string; mode: string }>;
		value: string;
		onChange: (value: string) => void;
	}) => (
		<div>
			<div>{`active:${value}`}</div>
			{options.map((option) => (
				<button
					key={option.mode}
					type="button"
					onClick={() => onChange(option.mode)}
				>
					{option.label ?? option.labelKey}
				</button>
			))}
		</div>
	),
}));

vi.mock("@/components/files/preview/PreviewUnavailable", () => ({
	PreviewUnavailable: () => <div>preview-unavailable</div>,
}));

vi.mock("@/components/files/preview/VideoPreview", () => ({
	VideoPreview: ({ path }: { path: string }) => <div>{`video:${path}`}</div>,
}));

vi.mock("@/components/files/preview/UnsavedChangesGuard", () => ({
	UnsavedChangesGuard: ({
		open,
		onOpenChange,
		onConfirm,
	}: {
		open: boolean;
		onOpenChange: (open: boolean) => void;
		onConfirm: () => void;
	}) =>
		open ? (
			<div>
				<div>unsaved-guard</div>
				<button type="button" onClick={() => onOpenChange(false)}>
					cancel-guard
				</button>
				<button type="button" onClick={onConfirm}>
					discard-changes
				</button>
			</div>
		) : null,
}));

vi.mock("@/components/files/preview/PdfPreview", () => ({
	PdfPreview: ({ path, fileName }: { path: string; fileName: string }) => (
		<div>{`pdf:${fileName}:${path}`}</div>
	),
}));

vi.mock("@/components/files/preview/MarkdownPreview", () => ({
	MarkdownPreview: ({ path }: { path: string }) => (
		<div>{`markdown:${path}`}</div>
	),
}));

vi.mock("@/components/files/preview/OfficeOnlinePreview", () => ({
	OfficeOnlinePreview: ({ downloadPath }: { downloadPath: string }) => (
		<div>{`office:${downloadPath}`}</div>
	),
}));

vi.mock("@/components/files/preview/CsvTablePreview", () => ({
	CsvTablePreview: ({
		path,
		delimiter,
	}: {
		path: string;
		delimiter: string;
	}) => <div>{`table:${delimiter}:${path}`}</div>,
}));

vi.mock("@/components/files/preview/JsonPreview", () => ({
	JsonPreview: ({ path }: { path: string }) => <div>{`json:${path}`}</div>,
}));

vi.mock("@/components/files/preview/XmlPreview", () => ({
	XmlPreview: ({ path, mode }: { path: string; mode: string }) => (
		<div>{`xml:${mode}:${path}`}</div>
	),
}));

vi.mock("@/components/files/preview/TextCodePreview", () => ({
	TextCodePreview: ({
		path,
		editable,
		onDirtyChange,
	}: {
		path: string;
		editable: boolean;
		onDirtyChange: (dirty: boolean) => void;
	}) => (
		<div>
			<div>{`code:${path}:${String(editable)}`}</div>
			<button type="button" onClick={() => onDirtyChange(true)}>
				mark-dirty
			</button>
		</div>
	),
}));

function renderDialog(
	overrides: Partial<React.ComponentProps<typeof FilePreviewDialog>> = {},
) {
	const onClose = vi.fn();
	const onFileUpdated = vi.fn();

	render(
		<FilePreviewDialog
			file={
				{
					id: 7,
					mime_type: "text/markdown",
					name: "notes.md",
					size: 128,
				} as never
			}
			onClose={onClose}
			onFileUpdated={onFileUpdated}
			editable
			{...overrides}
		/>,
	);

	return { onClose, onFileUpdated };
}

describe("FilePreviewDialog", () => {
	beforeEach(() => {
		mockState.downloadPath.mockClear();
		mockState.getStoredPreference.mockReset();
		mockState.setStoredPreference.mockReset();
		mockState.profile = {
			category: "markdown",
			defaultMode: "code",
			isBlobPreview: false,
			isEditableText: true,
			isTextBased: true,
			options: [
				{ icon: "TextT", labelKey: "mode_code", mode: "code" },
				{ icon: "MarkdownLogo", labelKey: "mode_markdown", mode: "markdown" },
			],
		};
		mockState.getStoredPreference.mockReturnValue(null);
		mockState.videoBrowserOption = null;
		mockState.videoBrowserTarget = null;
	});

	it("uses a valid stored preference and the default download path", async () => {
		mockState.getStoredPreference.mockReturnValueOnce("markdown");

		renderDialog();

		expect(mockState.getStoredPreference).toHaveBeenCalledWith("markdown");
		expect(mockState.downloadPath).toHaveBeenCalledWith(7);
		expect(
			await screen.findByText("markdown:/files/7/download"),
		).toBeInTheDocument();
		expect(screen.getByText("modes:2")).toBeInTheDocument();
		expect(screen.getByText("active:markdown")).toBeInTheDocument();
	});

	it("keeps a fixed work area for editor-style previews", async () => {
		renderDialog();

		await screen.findByText("code:/files/7/download:true");
		expect(
			screen.getByTestId("dialog-content").className.split(/\s+/),
		).toContain("h-[90vh]");
	});

	it("switches modes immediately when the editor is clean and persists the choice", async () => {
		renderDialog();

		expect(
			await screen.findByText("code:/files/7/download:true"),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "mode_markdown" }));

		await screen.findByText("markdown:/files/7/download");
		expect(mockState.setStoredPreference).toHaveBeenCalledWith(
			"markdown",
			"markdown",
		);
		expect(screen.getByText("active:markdown")).toBeInTheDocument();
	});

	it("guards mode switches when there are unsaved changes and applies them after discard", async () => {
		renderDialog();

		await screen.findByText("code:/files/7/download:true");
		fireEvent.click(screen.getByRole("button", { name: "mark-dirty" }));
		fireEvent.click(screen.getByRole("button", { name: "mode_markdown" }));

		expect(screen.getByText("unsaved-guard")).toBeInTheDocument();
		expect(mockState.setStoredPreference).not.toHaveBeenCalled();

		fireEvent.click(screen.getByRole("button", { name: "discard-changes" }));

		await screen.findByText("markdown:/files/7/download");
		expect(mockState.setStoredPreference).toHaveBeenCalledWith(
			"markdown",
			"markdown",
		);
	});

	it("guards closing when dirty and only closes after discard confirmation", async () => {
		const { onClose } = renderDialog();

		await screen.findByText("code:/files/7/download:true");
		fireEvent.click(screen.getByRole("button", { name: "mark-dirty" }));
		fireEvent.click(screen.getByRole("button", { name: "X" }));

		expect(screen.getByText("unsaved-guard")).toBeInTheDocument();
		expect(onClose).not.toHaveBeenCalled();

		fireEvent.click(screen.getByRole("button", { name: "discard-changes" }));

		await waitFor(() => {
			expect(onClose).toHaveBeenCalledTimes(1);
		});
	});

	it("falls back to preview unavailable when the profile has no active mode", async () => {
		mockState.profile = {
			category: "unknown",
			defaultMode: null,
			isBlobPreview: false,
			isEditableText: false,
			isTextBased: false,
			options: [],
		};

		renderDialog();

		expect(await screen.findByText("preview-unavailable")).toBeInTheDocument();
		expect(screen.getByText("modes:0")).toBeInTheDocument();
	});

	it("adds the configured custom video browser as an extra video open mode", async () => {
		mockState.profile = {
			category: "video",
			defaultMode: "video",
			isBlobPreview: true,
			isEditableText: false,
			isTextBased: false,
			options: [
				{ icon: "Monitor", labelKey: "open_with_video", mode: "video" },
			],
		};
		mockState.videoBrowserOption = {
			icon: "Globe",
			label: "Jellyfin",
			labelKey: "open_with_custom_video_browser",
			mode: "videoBrowser",
		};
		mockState.videoBrowserTarget = {
			label: "Jellyfin",
			mode: "iframe",
			url: "https://videos.example.com/watch?id=7",
		};

		renderDialog({
			file: {
				id: 7,
				mime_type: "video/mp4",
				name: "clip.mp4",
				size: 2048,
			} as never,
		});

		expect(
			await screen.findByText("video:/files/7/download"),
		).toBeInTheDocument();
		expect(screen.getByText("modes:2")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "Jellyfin" }));

		await screen.findByText(
			"video-browser:https://videos.example.com/watch?id=7",
		);
		expect(mockState.setStoredPreference).toHaveBeenCalledWith(
			"video",
			"videoBrowser",
		);
		expect(screen.getByText("active:videoBrowser")).toBeInTheDocument();
	});

	it("lets plain video previews size the dialog from their content", async () => {
		mockState.profile = {
			category: "video",
			defaultMode: "video",
			isBlobPreview: true,
			isEditableText: false,
			isTextBased: false,
			options: [
				{ icon: "Monitor", labelKey: "open_with_video", mode: "video" },
			],
		};

		renderDialog({
			file: {
				id: 7,
				mime_type: "video/mp4",
				name: "clip.mp4",
				size: 2048,
			} as never,
		});

		await screen.findByText("video:/files/7/download");
		const classes = screen.getByTestId("dialog-content").className.split(/\s+/);
		expect(classes).toContain("max-h-[90vh]");
		expect(classes).not.toContain("h-[90vh]");
	});

	it("lets image previews size to content without forcing a fixed-height work area", async () => {
		mockState.profile = {
			category: "image",
			defaultMode: "image",
			isBlobPreview: true,
			isEditableText: false,
			isTextBased: false,
			options: [{ icon: "Eye", labelKey: "open_with_image", mode: "image" }],
		};

		renderDialog({
			file: {
				id: 7,
				mime_type: "image/png",
				name: "tall-image.png",
				size: 2048,
			} as never,
		});

		await screen.findByText("blob:image:/files/7/download");
		expect(
			screen.getByTestId("dialog-content").className.split(/\s+/),
		).not.toContain("h-[90vh]");
	});

	it("renders office previews in the fixed-height workspace when a preview link factory exists", async () => {
		mockState.profile = {
			category: "document",
			defaultMode: "officeOnline",
			isBlobPreview: false,
			isEditableText: false,
			isTextBased: false,
			options: [
				{
					icon: "Globe",
					labelKey: "open_with_office_online",
					mode: "officeOnline",
				},
			],
		};

		renderDialog({
			file: {
				id: 7,
				mime_type:
					"application/vnd.openxmlformats-officedocument.wordprocessingml.document",
				name: "report.docx",
				size: 2048,
			} as never,
			previewLinkFactory: vi.fn(async () => ({
				expires_at: "2026-04-08T12:00:00Z",
				max_uses: 5,
				path: "/pv/token/report.docx",
			})),
		});

		expect(
			await screen.findByText("office:/files/7/download"),
		).toBeInTheDocument();
		expect(
			screen.getByTestId("dialog-content").className.split(/\s+/),
		).toContain("h-[90vh]");
	});

	it("falls back when office preview is selected without a preview link factory", async () => {
		mockState.profile = {
			category: "document",
			defaultMode: "officeOnline",
			isBlobPreview: false,
			isEditableText: false,
			isTextBased: false,
			options: [
				{
					icon: "Globe",
					labelKey: "open_with_office_online",
					mode: "officeOnline",
				},
			],
		};

		renderDialog({
			file: {
				id: 7,
				mime_type:
					"application/vnd.openxmlformats-officedocument.wordprocessingml.document",
				name: "report.docx",
				size: 2048,
			} as never,
		});

		expect(await screen.findByText("preview-unavailable")).toBeInTheDocument();
	});
});
