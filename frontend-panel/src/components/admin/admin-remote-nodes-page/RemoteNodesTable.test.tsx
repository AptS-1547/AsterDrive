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
		items,
		headerRow,
		renderRow,
	}: {
		items: RemoteNodeInfo[];
		headerRow: React.ReactNode;
		renderRow: (item: RemoteNodeInfo) => React.ReactNode;
	}) => (
		<table>
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
	is_enabled: true,
	enrollment_status: "not_started",
	last_error: "",
	last_checked_at: null,
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

describe("RemoteNodesTable", () => {
	it("disables the enrollment command action after enrollment completes", () => {
		const onGenerateEnrollmentCommand = vi.fn();

		render(
			<RemoteNodesTable
				generatingEnrollmentId={null}
				items={[remoteNode({ enrollment_status: "completed" })]}
				loading={false}
				onEdit={vi.fn()}
				onGenerateEnrollmentCommand={onGenerateEnrollmentCommand}
				onRequestDelete={vi.fn()}
			/>,
		);

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

		render(
			<RemoteNodesTable
				generatingEnrollmentId={null}
				items={[node]}
				loading={false}
				onEdit={vi.fn()}
				onGenerateEnrollmentCommand={onGenerateEnrollmentCommand}
				onRequestDelete={vi.fn()}
			/>,
		);

		fireEvent.click(
			screen.getByRole("button", {
				name: "remote_node_generate_enrollment_command",
			}),
		);

		expect(onGenerateEnrollmentCommand).toHaveBeenCalledWith(node);
	});
});
