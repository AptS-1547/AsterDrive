import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RemoteNodesTable } from "@/components/admin/admin-remote-nodes-page/RemoteNodesTable";
import type { RemoteNodeInfo } from "@/types/api";

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
	}),
}));

vi.mock("@/components/common/AdminTableList", () => ({
	AdminTableList: ({
		loading,
		items,
		headerRow,
		renderRow,
	}: {
		loading: boolean;
		items: RemoteNodeInfo[];
		headerRow: React.ReactNode;
		renderRow: (item: RemoteNodeInfo) => React.ReactNode;
	}) => (
		<table>
			<caption>{loading ? "loading" : "ready"}</caption>
			{headerRow}
			<tbody>{items.map(renderRow)}</tbody>
		</table>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => (
		<span aria-hidden="true" data-icon-name={name} />
	),
}));

vi.mock("@/components/ui/tooltip", () => ({
	Tooltip: ({ children }: { children: React.ReactNode }) => children,
	TooltipContent: ({ children }: { children: React.ReactNode }) => (
		<div role="tooltip">{children}</div>
	),
	TooltipProvider: ({ children }: { children: React.ReactNode }) => children,
	TooltipTrigger: ({ children }: { children: React.ReactNode }) => children,
}));

const remoteNode = (
	overrides: Partial<RemoteNodeInfo> = {},
): RemoteNodeInfo => ({
	id: 7,
	name: "Edge Alpha",
	base_url: "https://edge.example.com",
	transport_mode: "direct",
	is_enabled: true,
	enrollment_status: "not_started",
	last_error: "",
	last_checked_at: null,
	tunnel: {
		status: "offline",
		last_error: "",
		last_seen_at: null,
	},
	capabilities: {
		protocol_version: "v1",
		supports_list: true,
		supports_range_read: true,
		supports_stream_upload: true,
	},
	created_at: "",
	updated_at: "",
	...overrides,
});

function renderTable(
	props: Partial<React.ComponentProps<typeof RemoteNodesTable>> = {},
) {
	const defaultProps: React.ComponentProps<typeof RemoteNodesTable> = {
		deletingRemoteNodeId: null,
		generatingEnrollmentId: null,
		items: [remoteNode()],
		loading: false,
		onEdit: vi.fn(),
		onGenerateEnrollmentCommand: vi.fn(),
		onRequestDelete: vi.fn(),
		onSortChange: vi.fn(),
		sortBy: "id",
		sortOrder: "asc",
	};

	return render(<RemoteNodesTable {...defaultProps} {...props} />);
}

describe("RemoteNodesTable", () => {
	it("disables the enrollment command action after enrollment completes", () => {
		const onGenerateEnrollmentCommand = vi.fn();

		renderTable({
			items: [remoteNode({ enrollment_status: "completed" })],
			onGenerateEnrollmentCommand,
		});

		expect(
			screen.getByText("remote_node_enrollment_status_completed"),
		).toBeInTheDocument();
		const button = screen.getByRole("button", {
			name: "remote_node_enrollment_completed_action_disabled",
		});
		expect(button).toBeDisabled();

		fireEvent.click(button);

		expect(onGenerateEnrollmentCommand).not.toHaveBeenCalled();
	});

	it("keeps the enrollment command action available before completion", () => {
		const node = remoteNode({ enrollment_status: "pending" });
		const onGenerateEnrollmentCommand = vi.fn();

		renderTable({
			items: [node],
			onGenerateEnrollmentCommand,
		});

		fireEvent.click(
			screen.getByRole("button", {
				name: "remote_node_generate_enrollment_command",
			}),
		);

		expect(onGenerateEnrollmentCommand).toHaveBeenCalledWith(node);
	});

	it("marks reverse tunnel rows as test transport", () => {
		renderTable({
			items: [
				remoteNode({
					base_url: "",
					transport_mode: "reverse_tunnel",
				}),
			],
		});

		expect(
			screen.getByText("remote_node_transport_reverse_tunnel"),
		).toBeInTheDocument();
		expect(
			screen.getByText("remote_node_transport_test_badge"),
		).toBeInTheDocument();
	});

	it("opens rows by click and keyboard while ignoring rows being deleted", () => {
		const editableNode = remoteNode({ id: 7, name: "Editable" });
		const deletingNode = remoteNode({ id: 8, name: "Deleting" });
		const onEdit = vi.fn();

		renderTable({
			deletingRemoteNodeId: deletingNode.id,
			items: [editableNode, deletingNode],
			onEdit,
		});

		const editableRow = screen.getByText("Editable").closest("tr");
		const deletingRow = screen.getByText("Deleting").closest("tr");
		expect(editableRow).not.toBeNull();
		expect(deletingRow).not.toBeNull();

		fireEvent.click(editableRow as HTMLElement);
		fireEvent.keyDown(editableRow as HTMLElement, { key: "Enter" });
		fireEvent.keyDown(editableRow as HTMLElement, { key: " " });
		fireEvent.click(deletingRow as HTMLElement);
		fireEvent.keyDown(deletingRow as HTMLElement, { key: "Enter" });

		expect(onEdit).toHaveBeenCalledTimes(3);
		expect(onEdit).toHaveBeenCalledWith(editableNode);
	});

	it("shows deleting and generating states without firing disabled actions", () => {
		const node = remoteNode({
			enrollment_status: "pending",
			last_error: "storage unavailable",
			last_checked_at: "2026-05-29T08:00:00Z",
		});
		const onGenerateEnrollmentCommand = vi.fn();
		const onRequestDelete = vi.fn();

		renderTable({
			deletingRemoteNodeId: node.id,
			generatingEnrollmentId: node.id,
			items: [node],
			onGenerateEnrollmentCommand,
			onRequestDelete,
		});

		expect(
			screen.getByRole("button", {
				name: "remote_node_generate_enrollment_command",
			}),
		).toBeDisabled();
		expect(
			screen.getByRole("button", { name: "remote_node_deleting" }),
		).toBeDisabled();
		expect(screen.getAllByRole("tooltip").length).toBeGreaterThan(0);

		fireEvent.click(
			screen.getByRole("button", {
				name: "remote_node_generate_enrollment_command",
			}),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "remote_node_deleting" }),
		);

		expect(onGenerateEnrollmentCommand).not.toHaveBeenCalled();
		expect(onRequestDelete).not.toHaveBeenCalled();
	});

	it("requests deletion from the row action", () => {
		const onRequestDelete = vi.fn();

		renderTable({ onRequestDelete });

		fireEvent.click(screen.getByRole("button", { name: "delete_remote_node" }));

		expect(onRequestDelete).toHaveBeenCalledWith(7);
	});
});
