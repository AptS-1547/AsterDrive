import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TrashTable } from "@/components/trash/TrashTable";
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
		title,
	}: {
		children: React.ReactNode;
		className?: string;
		onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
		title?: string;
	}) => (
		<button type="button" className={className} onClick={onClick} title={title}>
			{children}
		</button>
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

describe("TrashTable", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it("renders file and folder rows with their metadata", () => {
		render(
			<TrashTable
				items={[createFileItem(), createFolderItem()]}
				allSelected
				selectedKeys={new Set(["file:1"])}
				onToggleSelectAll={vi.fn()}
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
		expect(screen.getByText("date:2026-03-28T00:00:00Z")).toBeInTheDocument();
		expect(screen.getByText("bytes:12")).toBeInTheDocument();
		expect(screen.getByText("Projects")).toBeInTheDocument();
		expect(screen.getByText("files:root")).toBeInTheDocument();
		expect(screen.getByText("icon:Folder")).toBeInTheDocument();
		expect(screen.getByText("—")).toBeInTheDocument();
		expect(screen.getAllByLabelText("checkbox:true")).toHaveLength(2);
	});

	it("toggles all rows and single rows from the table controls", () => {
		const item = createFileItem();
		const onToggleSelectAll = vi.fn();
		const onToggleSelect = vi.fn();

		render(
			<TrashTable
				items={[item]}
				allSelected={false}
				selectedKeys={new Set()}
				onToggleSelectAll={onToggleSelectAll}
				onToggleSelect={onToggleSelect}
				onRestore={vi.fn()}
				onPurge={vi.fn()}
			/>,
		);

		const checkboxes = screen.getAllByRole("button", { name: /checkbox:/ });
		fireEvent.click(checkboxes[0]);
		fireEvent.click(
			screen.getByText("report.pdf").closest("tr") as HTMLElement,
		);
		fireEvent.click(checkboxes[1]);

		expect(onToggleSelectAll).toHaveBeenCalledTimes(1);
		expect(onToggleSelect).toHaveBeenCalledTimes(2);
		expect(onToggleSelect).toHaveBeenNthCalledWith(1, item);
	});

	it("runs restore and purge actions without toggling the row selection", () => {
		const item = createFileItem();
		const onToggleSelect = vi.fn();
		const onRestore = vi.fn();
		const onPurge = vi.fn();

		render(
			<TrashTable
				items={[item]}
				allSelected={false}
				selectedKeys={new Set()}
				onToggleSelectAll={vi.fn()}
				onToggleSelect={onToggleSelect}
				onRestore={onRestore}
				onPurge={onPurge}
			/>,
		);

		fireEvent.click(screen.getByTitle("admin:restore"));
		fireEvent.click(screen.getByTitle("files:trash_delete_permanently"));

		expect(onRestore).toHaveBeenCalledWith(item);
		expect(onPurge).toHaveBeenCalledWith(item);
		expect(onToggleSelect).not.toHaveBeenCalled();
	});
});
