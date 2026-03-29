import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import MySharesPage from "@/pages/MySharesPage";
import type { MyShareInfo } from "@/types/api";

const mockState = vi.hoisted(() => ({
	deleteShare: vi.fn(),
	handleApiError: vi.fn(),
	listMine: vi.fn(),
	openWindow: vi.fn(),
	toastSuccess: vi.fn(),
	writeText: vi.fn(async () => undefined),
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, opts?: Record<string, unknown>) => {
			if (key === "share:my_shares_delete_title") {
				return `${key}:${opts?.name}`;
			}
			if (key === "share:my_shares_pagination_desc") {
				return `${key}:${opts?.current}/${opts?.total}/${opts?.count}`;
			}
			if (opts?.date) return `${key}:${opts.date}`;
			return key;
		},
	}),
}));

vi.mock("sonner", () => ({
	toast: {
		success: mockState.toastSuccess,
	},
}));

vi.mock("@/components/common/ConfirmDialog", () => ({
	ConfirmDialog: (props: {
		confirmLabel: string;
		description: string;
		onConfirm: () => void;
		onOpenChange: (open: boolean) => void;
		open: boolean;
		title: string;
	}) =>
		props.open ? (
			<div>
				<div>{props.title}</div>
				<div>{props.description}</div>
				<button type="button" onClick={props.onConfirm}>
					{props.confirmLabel}
				</button>
				<button type="button" onClick={() => props.onOpenChange(false)}>
					close
				</button>
			</div>
		) : null,
}));

vi.mock("@/components/common/EmptyState", () => ({
	EmptyState: (props: { description: string; title: string }) => (
		<div>{`${props.title}:${props.description}`}</div>
	),
}));

vi.mock("@/components/layout/AppLayout", () => ({
	AppLayout: (props: { children: React.ReactNode }) => (
		<div>{props.children}</div>
	),
}));

vi.mock("@/components/ui/badge", () => ({
	Badge: (props: { children: React.ReactNode }) => (
		<span>{props.children}</span>
	),
}));

vi.mock("@/components/files/FileTypeIcon", () => ({
	FileTypeIcon: (props: { fileName?: string; mimeType: string }) => (
		<span>{`file-icon:${props.fileName ?? ""}:${props.mimeType}`}</span>
	),
}));

vi.mock("@/components/ui/button", () => ({
	Button: (props: {
		children: React.ReactNode;
		disabled?: boolean;
		onClick?: () => void;
	}) => (
		<button type="button" disabled={props.disabled} onClick={props.onClick}>
			{props.children}
		</button>
	),
}));

vi.mock("@/components/ui/card", () => ({
	Card: (props: {
		children: React.ReactNode;
		className?: string;
		onClick?: () => void;
	}) => (
		<button type="button" className={props.className} onClick={props.onClick}>
			{props.children}
		</button>
	),
}));

vi.mock("@/components/ui/context-menu", () => ({
	ContextMenu: (props: { children: React.ReactNode }) => (
		<div>{props.children}</div>
	),
	ContextMenuContent: (props: { children: React.ReactNode }) => (
		<div>{props.children}</div>
	),
	ContextMenuItem: (props: {
		children: React.ReactNode;
		onClick?: () => void;
	}) => (
		<button type="button" onClick={props.onClick}>
			{props.children}
		</button>
	),
	ContextMenuSeparator: () => <hr />,
	ContextMenuTrigger: (props: { children: React.ReactNode }) => (
		<div>{props.children}</div>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: (props: { name: string }) => <span>{`icon:${props.name}`}</span>,
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: mockState.handleApiError,
}));

vi.mock("@/lib/format", () => ({
	formatDateAbsolute: (value: string) => `fmt:${value}`,
}));

vi.mock("@/services/shareService", () => ({
	shareService: {
		delete: mockState.deleteShare,
		listMine: mockState.listMine,
	},
}));

function createShare(overrides: Partial<MyShareInfo> = {}): MyShareInfo {
	return {
		id: 1,
		token: "token-1",
		resource_name: "Document.pdf",
		resource_type: "file",
		status: "active",
		has_password: false,
		created_at: "2026-03-28T00:00:00Z",
		expires_at: null,
		...overrides,
	} as MyShareInfo;
}

describe("MySharesPage", () => {
	beforeEach(() => {
		mockState.deleteShare.mockReset();
		mockState.deleteShare.mockResolvedValue(undefined);
		mockState.handleApiError.mockReset();
		mockState.listMine.mockReset();
		mockState.openWindow.mockReset();
		mockState.toastSuccess.mockReset();
		mockState.writeText.mockReset();
		mockState.writeText.mockResolvedValue(undefined);

		Object.defineProperty(navigator, "clipboard", {
			configurable: true,
			value: {
				writeText: mockState.writeText,
			},
		});

		Object.defineProperty(window, "open", {
			configurable: true,
			value: mockState.openWindow,
		});
	});

	it("loads shares, paginates forward, and returns to the previous page after deleting the last item", async () => {
		mockState.listMine
			.mockResolvedValueOnce({
				items: [
					createShare({ id: 1, resource_name: "Page One", token: "page-1" }),
				],
				total: 51,
			})
			.mockResolvedValueOnce({
				items: [
					createShare({ id: 51, resource_name: "Last Item", token: "page-2" }),
				],
				total: 51,
			})
			.mockResolvedValueOnce({
				items: [
					createShare({ id: 1, resource_name: "Page One", token: "page-1" }),
				],
				total: 50,
			});

		render(<MySharesPage />);

		await screen.findByText("Page One");
		expect(mockState.listMine).toHaveBeenNthCalledWith(1, {
			limit: 50,
			offset: 0,
		});

		fireEvent.click(screen.getByText("share:my_shares_next"));

		await screen.findByText("Last Item");
		expect(mockState.listMine).toHaveBeenNthCalledWith(2, {
			limit: 50,
			offset: 50,
		});

		fireEvent.click(screen.getByText("share:my_shares_card_delete"));

		await screen.findByText("share:my_shares_delete_title:Last Item");
		fireEvent.click(screen.getByText("delete"));

		await waitFor(() => {
			expect(mockState.deleteShare).toHaveBeenCalledWith(51);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith(
			"share:my_shares_delete_success",
		);
		await waitFor(() => {
			expect(mockState.listMine).toHaveBeenNthCalledWith(3, {
				limit: 50,
				offset: 0,
			});
		});
	});

	it("copies and opens share links from card actions", async () => {
		mockState.listMine.mockResolvedValue({
			items: [
				createShare({
					id: 7,
					resource_name: "Doc.pdf",
					token: "token-doc",
				}),
			],
			total: 1,
		});

		render(<MySharesPage />);

		await screen.findByText("Doc.pdf");

		fireEvent.click(screen.getByText("share:my_shares_card_copy"));

		await waitFor(() => {
			expect(mockState.writeText).toHaveBeenCalledWith(
				"http://localhost:3000/s/token-doc",
			);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("copied_to_clipboard");

		fireEvent.click(screen.getByText("share:my_shares_card_open"));

		expect(mockState.openWindow).toHaveBeenCalledWith(
			"/s/token-doc",
			"_blank",
			"noopener,noreferrer",
		);
	});

	it("uses file-browser style icons for file and folder shares", async () => {
		mockState.listMine.mockResolvedValue({
			items: [
				createShare({
					id: 8,
					resource_name: "Document.pdf",
					resource_type: "file",
				}),
				createShare({
					id: 9,
					resource_name: "Projects",
					resource_type: "folder",
				}),
			],
			total: 2,
		});

		render(<MySharesPage />);

		await screen.findByText("Document.pdf");
		expect(screen.getByText("file-icon:Document.pdf:")).toBeInTheDocument();
		expect(screen.getByText("icon:Folder")).toBeInTheDocument();
	});
});
