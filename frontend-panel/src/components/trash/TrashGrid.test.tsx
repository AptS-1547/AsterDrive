import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TrashGrid } from "@/components/trash/TrashGrid";
import type { TrashItem } from "@/types/api-helpers";

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
		i18n: {
			language: "zh",
			resolvedLanguage: "zh",
			t: (key: string) => key,
		},
	}),
}));

vi.mock("@/components/files/FileTypeIcon", () => ({
	FileTypeIcon: ({
		fileName,
		mimeType,
	}: {
		fileName: string;
		mimeType: string;
	}) => <span>{`file-icon:${fileName}:${mimeType}`}</span>,
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		className,
		onClick,
	}: {
		children: React.ReactNode;
		className?: string;
		onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
	}) => (
		<button type="button" className={className} onClick={onClick}>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/card", () => ({
	Card: ({
		children,
		className,
		onClick,
		onKeyDown,
		tabIndex,
	}: {
		children: React.ReactNode;
		className?: string;
		onClick?: () => void;
		onKeyDown?: (event: React.KeyboardEvent<HTMLDivElement>) => void;
		tabIndex?: number;
	}) => (
		// biome-ignore lint/a11y/useSemanticElements: test double needs nested action buttons inside the interactive wrapper
		<div
			data-testid="trash-card"
			className={className}
			onClick={onClick}
			onKeyDown={onKeyDown}
			role="button"
			tabIndex={tabIndex}
		>
			{children}
		</div>
	),
	CardContent: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	CardFooter: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{`icon:${name}`}</span>,
}));

vi.mock("@/components/ui/item-checkbox", () => ({
	ItemCheckbox: ({
		checked,
		onChange,
	}: {
		checked: boolean;
		onChange: () => void;
	}) => (
		<button
			type="button"
			aria-label={`checkbox:${checked}`}
			onClick={(event) => {
				event.stopPropagation();
				onChange();
			}}
		/>
	),
}));

vi.mock("@/lib/format", () => ({
	formatBytes: (value: number) => `bytes:${value}`,
	formatDate: (value: string) => `date:${value}`,
	formatDateAbsolute: (value: string) => `absolute:${value}`,
}));

function createFileItem(overrides: Partial<TrashItem> = {}) {
	return {
		deleted_at: "2026-03-28T00:00:00Z",
		entity_type: "file",
		id: 1,
		mime_type: "application/pdf",
		name: "report.pdf",
		original_path: "/Docs",
		size: 12,
		...overrides,
	} as TrashItem;
}

function createFolderItem(overrides: Partial<TrashItem> = {}) {
	return {
		deleted_at: "2026-03-29T00:00:00Z",
		entity_type: "folder",
		id: 2,
		name: "Projects",
		original_path: "/",
		size: 0,
		...overrides,
	} as TrashItem;
}

describe("TrashGrid", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it("renders file and folder metadata with translated root paths", () => {
		render(
			<TrashGrid
				items={[createFileItem(), createFolderItem()]}
				selectedKeys={new Set(["file:1"])}
				onToggleSelect={vi.fn()}
				onRestore={vi.fn()}
				onPurge={vi.fn()}
			/>,
		);

		expect(screen.getByText("report.pdf")).toBeInTheDocument();
		expect(
			screen.getByText("file-icon:report.pdf:application/pdf"),
		).toBeInTheDocument();
		expect(screen.getByText("/Docs")).toBeInTheDocument();
		expect(screen.getByText("bytes:12")).toBeInTheDocument();
		expect(screen.getByText("date:2026-03-28T00:00:00Z")).toBeInTheDocument();
		expect(screen.getByText("Projects")).toBeInTheDocument();
		expect(screen.getByText("files:root")).toBeInTheDocument();
		expect(screen.getByText("icon:Folder")).toBeInTheDocument();
		expect(screen.getByText("—")).toBeInTheDocument();
		expect(screen.getByLabelText("checkbox:true")).toBeInTheDocument();
	});

	it("toggles selection from card clicks, keyboard shortcuts, and checkboxes", () => {
		const item = createFileItem();
		const onToggleSelect = vi.fn();

		render(
			<TrashGrid
				items={[item]}
				selectedKeys={new Set()}
				onToggleSelect={onToggleSelect}
				onRestore={vi.fn()}
				onPurge={vi.fn()}
			/>,
		);

		const card = screen
			.getByText("report.pdf")
			.closest("[data-testid='trash-card']");
		expect(card).not.toBeNull();

		fireEvent.click(card as HTMLElement);
		fireEvent.keyDown(card as HTMLElement, { key: "Enter" });
		fireEvent.keyDown(card as HTMLElement, { key: " " });
		fireEvent.click(screen.getByLabelText("checkbox:false"));

		expect(onToggleSelect).toHaveBeenCalledTimes(4);
		expect(onToggleSelect).toHaveBeenNthCalledWith(1, item);
	});

	it("runs restore and purge actions without toggling selection", () => {
		const item = createFileItem();
		const onToggleSelect = vi.fn();
		const onRestore = vi.fn();
		const onPurge = vi.fn();

		render(
			<TrashGrid
				items={[item]}
				selectedKeys={new Set()}
				onToggleSelect={onToggleSelect}
				onRestore={onRestore}
				onPurge={onPurge}
			/>,
		);

		fireEvent.click(screen.getByText("admin:restore"));
		fireEvent.click(screen.getByText("files:trash_delete_permanently"));

		expect(onRestore).toHaveBeenCalledWith(item);
		expect(onPurge).toHaveBeenCalledWith(item);
		expect(onToggleSelect).not.toHaveBeenCalled();
	});
});
