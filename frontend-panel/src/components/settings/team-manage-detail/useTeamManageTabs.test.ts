import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useTeamManageTabs } from "@/components/settings/team-manage-detail/useTeamManageTabs";

function renderTabs(
	initialProps: Parameters<typeof useTeamManageTabs>[0] = {
		canArchiveTeam: true,
		canManageTeam: true,
		detailLoading: false,
		detailRequestStarted: true,
		isPageLayout: false,
	},
) {
	return renderHook(
		(props: Parameters<typeof useTeamManageTabs>[0]) =>
			useTeamManageTabs(props),
		{
			initialProps,
		},
	);
}

describe("useTeamManageTabs", () => {
	it("manages allowed dialog tabs and resets when permissions narrow", () => {
		const hook = renderTabs();

		act(() => {
			hook.result.current.handleTabChange("audit");
		});

		expect(hook.result.current.currentTab).toBe("audit");

		hook.rerender({
			canArchiveTeam: false,
			canManageTeam: false,
			detailLoading: false,
			detailRequestStarted: true,
			isPageLayout: false,
		});

		expect(hook.result.current.currentTab).toBe("overview");

		act(() => {
			hook.result.current.handleTabChange("danger");
		});

		expect(hook.result.current.currentTab).toBe("overview");
	});

	it("syncs page tabs and redirects disallowed page tabs after detail loads", () => {
		const onPageTabChange = vi.fn();
		const hook = renderTabs({
			canArchiveTeam: true,
			canManageTeam: true,
			detailLoading: false,
			detailRequestStarted: true,
			isPageLayout: true,
			onPageTabChange,
			pageTab: "overview",
		});

		act(() => {
			hook.result.current.handleTabChange("danger");
		});

		expect(onPageTabChange).toHaveBeenCalledWith("danger");
		hook.rerender({
			canArchiveTeam: true,
			canManageTeam: true,
			detailLoading: false,
			detailRequestStarted: true,
			isPageLayout: true,
			onPageTabChange,
			pageTab: "danger",
		});

		expect(hook.result.current.currentTab).toBe("danger");
		expect(hook.result.current.panelAnimationClass).toContain(
			"slide-in-from-right-4",
		);

		hook.rerender({
			canArchiveTeam: false,
			canManageTeam: false,
			detailLoading: false,
			detailRequestStarted: true,
			isPageLayout: true,
			onPageTabChange,
			pageTab: "danger",
		});

		expect(onPageTabChange).toHaveBeenCalledWith("overview", {
			replace: true,
		});

		hook.rerender({
			canArchiveTeam: false,
			canManageTeam: false,
			detailLoading: false,
			detailRequestStarted: true,
			isPageLayout: true,
			onPageTabChange,
			pageTab: "overview",
		});

		expect(hook.result.current.currentTab).toBe("overview");
		expect(hook.result.current.panelAnimationClass).toContain(
			"slide-in-from-left-4",
		);
	});
});
