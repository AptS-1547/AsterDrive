import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
	UploadPanel,
	type UploadTaskView,
} from "@/components/files/UploadPanel";

vi.mock("@tanstack/react-virtual", () => ({
	useVirtualizer: ({
		count,
		estimateSize,
	}: {
		count: number;
		estimateSize: (index: number) => number;
	}) => {
		let start = 0;
		const items = Array.from({ length: count }, (_, index) => {
			const size = estimateSize(index);
			const item = { index, size, start };
			start += size;
			return item;
		});

		return {
			getTotalSize: () => start,
			getVirtualItems: () => items,
		};
	},
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, opts?: Record<string, unknown>) => {
			if (key === "root") return "Root";
			if (key === "upload_target_location") return "Target";
			if (key === "upload_target_current") return "Current";
			if (key === "upload_group_stats") {
				return `total:${opts?.total} success:${opts?.success} failed:${opts?.failed} active:${opts?.active}`;
			}
			return key;
		},
	}),
}));

function renderPanel(tasks: UploadTaskView[]) {
	const onClearCompleted = vi.fn();
	const onRetryFailed = vi.fn();
	const onToggle = vi.fn();

	render(
		<UploadPanel
			open
			onToggle={onToggle}
			title="Uploads"
			summary="3 tasks"
			tasks={tasks}
			emptyText="No tasks"
			totalCount={tasks.length}
			successCount={1}
			failedCount={1}
			activeCount={1}
			overallProgress={67}
			onRetryFailed={onRetryFailed}
			retryFailedLabel="Retry failed"
			onClearCompleted={onClearCompleted}
			clearCompletedLabel="Clear completed"
		/>,
	);

	return { onClearCompleted, onRetryFailed, onToggle };
}

describe("UploadPanel", () => {
	it("renders the empty state when there are no tasks", () => {
		render(
			<UploadPanel
				open
				onToggle={vi.fn()}
				title="Uploads"
				summary="0 tasks"
				tasks={[]}
				emptyText="No tasks"
			/>,
		);

		expect(screen.getByText("No tasks")).toBeInTheDocument();
	});

	it("groups tasks and exposes footer actions", () => {
		const tasks: UploadTaskView[] = [
			{
				id: "root-done",
				title: "done.txt",
				status: "Done",
				mode: "Direct",
				progress: 100,
				completed: true,
				targetLabel: "Projects",
			},
			{
				id: "folder-active",
				title: "active.txt",
				status: "Uploading",
				mode: "Chunked",
				progress: 55,
				group: "Folder A",
				targetLabel: "Projects",
			},
			{
				id: "folder-failed",
				title: "failed.txt",
				status: "Failed",
				mode: "Presigned",
				progress: 30,
				group: "Folder B",
				targetLabel: "Projects",
				actions: [
					{
						label: "Retry",
						icon: "ArrowsClockwise",
						onClick: vi.fn(),
					},
				],
			},
		];

		const { onClearCompleted, onRetryFailed, onToggle } = renderPanel(tasks);

		expect(screen.getByText("upload_stat_total")).toBeInTheDocument();
		expect(screen.getByText("3")).toBeInTheDocument();
		expect(screen.getByText("67%")).toBeInTheDocument();

		expect(screen.getByText("Root")).toBeInTheDocument();
		expect(screen.getByText("upload_batch_done")).toBeInTheDocument();
		expect(screen.getByText("Folder A")).toBeInTheDocument();
		expect(screen.getByText("upload_batch_active")).toBeInTheDocument();
		expect(screen.getByText("Folder B")).toBeInTheDocument();
		expect(screen.getByText("upload_batch_partial_failed")).toBeInTheDocument();
		expect(
			screen.getByText("total:1 success:1 failed:0 active:0"),
		).toBeInTheDocument();
		expect(
			screen.getByText("total:1 success:0 failed:0 active:1"),
		).toBeInTheDocument();
		expect(
			screen.getByText("total:1 success:0 failed:1 active:0"),
		).toBeInTheDocument();

		fireEvent.click(screen.getByText("Retry failed"));
		fireEvent.click(screen.getByText("Clear completed"));
		fireEvent.click(screen.getAllByRole("button")[0]);

		expect(onRetryFailed).toHaveBeenCalledTimes(1);
		expect(onClearCompleted).toHaveBeenCalledTimes(1);
		expect(onToggle).toHaveBeenCalledTimes(1);
	});
});
