import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TeamsSettingsView } from "@/components/settings/TeamsSettingsView";

const mockState = vi.hoisted(() => ({
	archivedList: vi.fn(),
	ensureTeamsLoaded: vi.fn(),
	handleApiError: vi.fn(),
	navigate: vi.fn(),
	reloadTeams: vi.fn(),
	restoreTeam: vi.fn(),
	toastSuccess: vi.fn(),
}));

const ACTIVE_TEAMS = [
	{
		archived_at: null,
		created_at: "2026-04-01T00:00:00Z",
		created_by: 2,
		created_by_username: "owner",
		description: "Design team",
		id: 11,
		member_count: 5,
		my_role: "owner",
		name: "Design",
		policy_group_id: 7,
		storage_quota: 1024,
		storage_used: 512,
		updated_at: "2026-04-02T00:00:00Z",
	},
] as const;

const ARCHIVED_TEAMS = [
	{
		archived_at: "2026-04-03T00:00:00Z",
		created_at: "2026-03-01T00:00:00Z",
		created_by: 3,
		created_by_username: "archiver",
		description: "Archived team",
		id: 23,
		member_count: 2,
		my_role: "admin",
		name: "Legacy",
		policy_group_id: 8,
		storage_quota: 2048,
		storage_used: 256,
		updated_at: "2026-04-03T00:00:00Z",
	},
] as const;

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => key,
	}),
}));

vi.mock("react-router-dom", () => ({
	useNavigate: () => mockState.navigate,
}));

vi.mock("sonner", () => ({
	toast: {
		success: (...args: unknown[]) => mockState.toastSuccess(...args),
	},
}));

vi.mock("@/components/settings/TeamManageDialog", () => ({
	TeamManageDialog: ({
		open,
		teamId,
	}: {
		open: boolean;
		teamId: number | null;
	}) => (open ? <div>{`manage:${teamId}`}</div> : null),
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: (...args: unknown[]) => mockState.handleApiError(...args),
}));

vi.mock("@/stores/authStore", () => ({
	useAuthStore: (selector: (state: { user: { id: number } }) => unknown) =>
		selector({ user: { id: 7 } }),
}));

vi.mock("@/stores/teamStore", () => ({
	useTeamStore: (
		selector: (state: {
			ensureLoaded: typeof mockState.ensureTeamsLoaded;
			loading: boolean;
			reload: typeof mockState.reloadTeams;
			teams: typeof ACTIVE_TEAMS;
		}) => unknown,
	) =>
		selector({
			ensureLoaded: mockState.ensureTeamsLoaded,
			loading: false,
			reload: mockState.reloadTeams,
			teams: ACTIVE_TEAMS,
		}),
}));

vi.mock("@/services/teamService", () => ({
	teamService: {
		list: (...args: unknown[]) => mockState.archivedList(...args),
		restore: (...args: unknown[]) => mockState.restoreTeam(...args),
	},
}));

describe("TeamsSettingsView", () => {
	beforeEach(() => {
		mockState.archivedList.mockReset();
		mockState.ensureTeamsLoaded.mockReset();
		mockState.handleApiError.mockReset();
		mockState.navigate.mockReset();
		mockState.reloadTeams.mockReset();
		mockState.restoreTeam.mockReset();
		mockState.toastSuccess.mockReset();

		mockState.archivedList.mockResolvedValue(ARCHIVED_TEAMS);
		mockState.ensureTeamsLoaded.mockResolvedValue(undefined);
		mockState.reloadTeams.mockResolvedValue(undefined);
		mockState.restoreTeam.mockResolvedValue({
			...ARCHIVED_TEAMS[0],
			archived_at: null,
		});
	});

	it("opens the team management dialog from the active team card", async () => {
		render(<TeamsSettingsView />);

		await screen.findByText("Design");
		expect(mockState.ensureTeamsLoaded).toHaveBeenCalledWith(7);
		expect(mockState.archivedList).toHaveBeenCalledWith({ archived: true });

		fireEvent.click(screen.getByRole("button", { name: "core:manage" }));

		expect(screen.getByText("manage:11")).toBeInTheDocument();
	});

	it("restores an archived team and opens its management dialog", async () => {
		render(<TeamsSettingsView />);

		await screen.findByText("Legacy");
		fireEvent.click(
			screen.getByRole("button", {
				name: "settings:settings_team_restore",
			}),
		);

		await waitFor(() => {
			expect(mockState.restoreTeam).toHaveBeenCalledWith(23);
		});
		expect(mockState.reloadTeams).toHaveBeenCalledWith(7);
		expect(mockState.toastSuccess).toHaveBeenCalledWith(
			"settings:settings_team_restored",
		);
		expect(screen.getByText("manage:23")).toBeInTheDocument();
	});
});
