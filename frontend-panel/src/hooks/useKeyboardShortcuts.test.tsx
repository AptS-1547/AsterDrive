import { fireEvent, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

interface MockFileStoreState {
	clipboard: { mode: "copy" | "cut" } | null;
	clipboardCopy: () => number;
	clipboardCut: () => number;
	clipboardPaste: () => Promise<{
		mode: "copy" | "move";
		result: {
			succeeded: number;
			failed: number;
			errors: [];
		};
	}>;
	clearSelection: () => void;
	files: Array<{ id: number; name: string }>;
	folders: Array<{ id: number; name: string }>;
	selectedFileIds: Set<number>;
	selectedFolderIds: Set<number>;
	selectAll: () => void;
}

const mockState = vi.hoisted(() => {
	const store: MockFileStoreState = {
		clipboard: null,
		clipboardCopy: vi.fn(() => 0),
		clipboardCut: vi.fn(() => 0),
		clipboardPaste: vi.fn(async () => ({
			mode: "copy",
			result: { succeeded: 1, failed: 0, errors: [] },
		})),
		clearSelection: vi.fn(),
		files: [],
		folders: [],
		selectedFileIds: new Set(),
		selectedFolderIds: new Set(),
		selectAll: vi.fn(),
	};

	return {
		formatBatchToast: vi.fn(() => ({
			variant: "success" as const,
			title: "batch success",
			description: "details",
		})),
		handleApiError: vi.fn(),
		shouldIgnoreKeyboardTarget: vi.fn(() => false),
		store,
		toastError: vi.fn(),
		toastInfo: vi.fn(),
		toastSuccess: vi.fn(),
		useSelectionShortcuts: vi.fn(),
	};
});

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, opts?: Record<string, unknown>) =>
			opts ? `${key}:${JSON.stringify(opts)}` : key,
	}),
}));

vi.mock("sonner", () => ({
	toast: {
		error: mockState.toastError,
		info: mockState.toastInfo,
		success: mockState.toastSuccess,
	},
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: mockState.handleApiError,
}));

vi.mock("@/hooks/useSelectionShortcuts", () => ({
	shouldIgnoreKeyboardTarget: mockState.shouldIgnoreKeyboardTarget,
	useSelectionShortcuts: mockState.useSelectionShortcuts,
}));

vi.mock("@/lib/formatBatchToast", () => ({
	formatBatchToast: mockState.formatBatchToast,
}));

vi.mock("@/stores/fileStore", () => {
	const useFileStore = <T,>(selector: (state: MockFileStoreState) => T) =>
		selector(mockState.store);
	useFileStore.getState = () => mockState.store;
	return { useFileStore };
});

describe("useKeyboardShortcuts", () => {
	beforeEach(() => {
		mockState.formatBatchToast.mockClear();
		mockState.handleApiError.mockReset();
		mockState.shouldIgnoreKeyboardTarget.mockReset();
		mockState.shouldIgnoreKeyboardTarget.mockReturnValue(false);
		mockState.store.clipboard = null;
		mockState.store.clipboardCopy = vi.fn(() => 0);
		mockState.store.clipboardCut = vi.fn(() => 0);
		mockState.store.clipboardPaste = vi.fn(async () => ({
			mode: "copy",
			result: { succeeded: 1, failed: 0, errors: [] },
		}));
		mockState.store.clearSelection = vi.fn();
		mockState.store.files = [];
		mockState.store.folders = [];
		mockState.store.selectedFileIds = new Set();
		mockState.store.selectedFolderIds = new Set();
		mockState.store.selectAll = vi.fn();
		mockState.toastError.mockReset();
		mockState.toastInfo.mockReset();
		mockState.toastSuccess.mockReset();
		mockState.useSelectionShortcuts.mockClear();
		document.body.innerHTML = "";
	});

	it("wires shared selection shortcuts from the file store", async () => {
		const { useKeyboardShortcuts } = await import(
			"@/hooks/useKeyboardShortcuts"
		);

		renderHook(() => useKeyboardShortcuts());

		expect(mockState.useSelectionShortcuts).toHaveBeenCalledWith({
			selectAll: mockState.store.selectAll,
			clearSelection: mockState.store.clearSelection,
		});
	});

	it("handles clipboard copy and cut shortcuts", async () => {
		const { useKeyboardShortcuts } = await import(
			"@/hooks/useKeyboardShortcuts"
		);
		mockState.store.clipboardCopy = vi.fn(() => 2);
		mockState.store.clipboardCut = vi.fn(() => 1);

		renderHook(() => useKeyboardShortcuts());

		fireEvent.keyDown(document, {
			ctrlKey: true,
			key: "c",
		});
		fireEvent.keyDown(document, {
			ctrlKey: true,
			key: "x",
		});

		expect(mockState.store.clipboardCopy).toHaveBeenCalledTimes(1);
		expect(mockState.store.clipboardCut).toHaveBeenCalledTimes(1);
		expect(mockState.toastInfo).toHaveBeenNthCalledWith(
			1,
			'clipboard_copied:{"count":2}',
		);
		expect(mockState.toastInfo).toHaveBeenNthCalledWith(
			2,
			'clipboard_cut:{"count":1}',
		);
	});

	it("ignores shortcuts while IME composition is active", async () => {
		const { useKeyboardShortcuts } = await import(
			"@/hooks/useKeyboardShortcuts"
		);
		mockState.store.clipboardCopy = vi.fn(() => 2);

		renderHook(() => useKeyboardShortcuts());

		fireEvent.keyDown(document, {
			ctrlKey: true,
			key: "c",
			keyCode: 229,
		});

		expect(mockState.store.clipboardCopy).not.toHaveBeenCalled();
		expect(mockState.toastInfo).not.toHaveBeenCalled();
	});

	it("formats and displays paste results", async () => {
		const { useKeyboardShortcuts } = await import(
			"@/hooks/useKeyboardShortcuts"
		);
		mockState.store.clipboard = { mode: "copy" };

		renderHook(() => useKeyboardShortcuts());

		fireEvent.keyDown(document, {
			metaKey: true,
			key: "v",
		});

		await waitFor(() => {
			expect(mockState.store.clipboardPaste).toHaveBeenCalledTimes(1);
		});
		expect(mockState.formatBatchToast).toHaveBeenCalledWith(
			expect.any(Function),
			"copy",
			{
				succeeded: 1,
				failed: 0,
				errors: [],
			},
		);
		expect(mockState.toastSuccess).toHaveBeenCalledWith("batch success", {
			description: "details",
		});
	});

	it("routes paste failures to handleApiError", async () => {
		const { useKeyboardShortcuts } = await import(
			"@/hooks/useKeyboardShortcuts"
		);
		const failure = new Error("paste failed");
		mockState.store.clipboard = { mode: "cut" };
		mockState.store.clipboardPaste = vi.fn(async () => {
			throw failure;
		});

		renderHook(() => useKeyboardShortcuts());

		fireEvent.keyDown(document, {
			ctrlKey: true,
			key: "v",
		});

		await waitFor(() => {
			expect(mockState.handleApiError).toHaveBeenCalledWith(failure);
		});
	});

	it("dispatches rename requests for the current single selection", async () => {
		const { useKeyboardShortcuts } = await import(
			"@/hooks/useKeyboardShortcuts"
		);
		const events: CustomEvent[] = [];
		document.addEventListener("rename-request", ((event: Event) => {
			events.push(event as CustomEvent);
		}) as EventListener);
		mockState.store.selectedFileIds = new Set([7]);
		mockState.store.files = [{ id: 7, name: "notes.txt" }];

		renderHook(() => useKeyboardShortcuts());

		fireEvent.keyDown(document, {
			key: "F2",
		});

		expect(events[0]?.detail).toEqual({
			type: "file",
			id: 7,
			name: "notes.txt",
		});
	});
});
