import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AdminTeamDetailDialog } from "@/components/admin/AdminTeamDetailDialog";

const mockState = vi.hoisted(() => ({
	handleApiError: vi.fn(),
}));

const adminTeamServiceMocks = vi.hoisted(() => ({
	addMember: vi.fn(),
	delete: vi.fn(),
	get: vi.fn(),
	listAuditLogs: vi.fn(),
	listMembers: vi.fn(),
	removeMember: vi.fn(),
	restore: vi.fn(),
	update: vi.fn(),
	updateMember: vi.fn(),
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
	}),
}));

vi.mock("sonner", () => ({
	toast: {
		error: vi.fn(),
		success: vi.fn(),
	},
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: (...args: unknown[]) => mockState.handleApiError(...args),
}));

vi.mock("@/services/adminService", () => ({
	adminTeamService: adminTeamServiceMocks,
}));

describe("AdminTeamDetailDialog", () => {
	beforeEach(() => {
		mockState.handleApiError.mockReset();
		adminTeamServiceMocks.addMember.mockReset();
		adminTeamServiceMocks.delete.mockReset();
		adminTeamServiceMocks.get.mockReset();
		adminTeamServiceMocks.listAuditLogs.mockReset();
		adminTeamServiceMocks.listMembers.mockReset();
		adminTeamServiceMocks.removeMember.mockReset();
		adminTeamServiceMocks.restore.mockReset();
		adminTeamServiceMocks.update.mockReset();
		adminTeamServiceMocks.updateMember.mockReset();

		adminTeamServiceMocks.get.mockResolvedValue({
			archived_at: null,
			created_at: "2026-04-01T00:00:00Z",
			created_by: 1,
			created_by_username: "root",
			description: "Team description",
			id: 14,
			member_count: 8,
			name: "Product",
			policy_group_id: 5,
			storage_quota: 1024,
			storage_used: 512,
			updated_at: "2026-04-02T00:00:00Z",
		});
		adminTeamServiceMocks.listMembers.mockResolvedValue({
			items: [],
			manager_count: 1,
			owner_count: 1,
			total: 0,
		});
		adminTeamServiceMocks.listAuditLogs.mockResolvedValue({
			items: [],
			total: 0,
		});
	});

	it("uses a fixed shell and a native scrollable detail column in page layout", async () => {
		const { container } = render(
			<AdminTeamDetailDialog
				layout="page"
				onListChange={async () => undefined}
				onOpenChange={vi.fn()}
				onPageTabChange={vi.fn()}
				onRefreshPolicyGroups={async () => undefined}
				open
				pageTab="overview"
				policyGroups={[
					{
						created_at: "2026-04-01T00:00:00Z",
						description: "",
						id: 5,
						is_default: false,
						is_enabled: true,
						items: [
							{
								id: 1,
								max_file_size: 0,
								min_file_size: 0,
								policy: {
									id: 7,
									name: "Default",
								},
								policy_id: 7,
								priority: 1,
							},
						],
						name: "Primary",
						updated_at: "2026-04-01T00:00:00Z",
					},
				]}
				policyGroupsLoading={false}
				teamId={14}
			/>,
		);

		await waitFor(() => {
			expect(adminTeamServiceMocks.get).toHaveBeenCalledWith(14);
			expect(adminTeamServiceMocks.listMembers).toHaveBeenCalled();
			expect(adminTeamServiceMocks.listAuditLogs).toHaveBeenCalled();
		});

		expect(
			container.querySelector(
				".flex.min-h-0.flex-1.flex-col.overflow-y-auto.lg\\:overflow-hidden",
			),
		).not.toBeNull();
		expect(
			container.querySelector(
				".flex.min-h-full.flex-col.lg\\:h-full.lg\\:min-h-0.lg\\:flex-1.lg\\:flex-row",
			),
		).not.toBeNull();
		expect(
			container.querySelector(
				".border-b.bg-muted\\/20.lg\\:min-h-0.lg\\:w-80.lg\\:flex-none.lg\\:overflow-y-auto",
			),
		).not.toBeNull();
		expect(
			container.querySelector(
				".min-h-0.min-w-0.lg\\:flex-1.lg\\:flex.lg\\:h-full.lg\\:flex-col.lg\\:overflow-hidden",
			),
		).not.toBeNull();
		expect(
			container.querySelector(
				".flex.flex-col.lg\\:h-full.lg\\:min-h-0.lg\\:flex-1.lg\\:overflow-hidden",
			),
		).not.toBeNull();
		expect(
			container.querySelector(
				".px-6.pt-4.pb-6.lg\\:min-h-0.lg\\:flex-1.lg\\:overflow-y-auto",
			),
		).not.toBeNull();
		expect(container.querySelector('[data-slot="scroll-area"]')).toBeNull();

		const membersTab = screen.getByRole("tab", {
			name: "settings:settings_team_members",
		});
		expect(membersTab).toHaveClass("min-w-0");
		expect(membersTab).not.toHaveClass("flex-none");
		expect(membersTab.parentElement).toHaveClass("w-full", "gap-5", "border-b");
		expect(membersTab.parentElement).not.toHaveClass("overflow-x-auto");
	});
});
