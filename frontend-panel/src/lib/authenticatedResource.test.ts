import { beforeEach, describe, expect, it, vi } from "vitest";

const mockState = vi.hoisted(() => ({
	debug: vi.fn(),
	ensureFreshSession: vi.fn(),
	get: vi.fn(),
}));

vi.mock("@/services/http", () => ({
	api: {
		client: {
			get: mockState.get,
		},
	},
}));

vi.mock("@/stores/authStore", () => ({
	useAuthStore: {
		getState: () => ({
			ensureFreshSession: mockState.ensureFreshSession,
		}),
	},
}));

vi.mock("@/lib/logger", () => ({
	logger: {
		debug: (...args: unknown[]) => mockState.debug(...args),
	},
}));

describe("prepareAuthenticatedResource", () => {
	beforeEach(() => {
		mockState.debug.mockReset();
		mockState.ensureFreshSession.mockReset();
		mockState.ensureFreshSession.mockResolvedValue(undefined);
		mockState.get.mockReset();
		mockState.get.mockResolvedValue({
			data: new Blob(["x"]),
			headers: {},
			status: 206,
		});
	});

	it("prepares protected API resources through the shared auth refresh path", async () => {
		const { prepareAuthenticatedResource } = await import(
			"@/lib/authenticatedResource"
		);

		await prepareAuthenticatedResource("/files/7/download");

		expect(mockState.ensureFreshSession).toHaveBeenCalledTimes(1);
		expect(mockState.get).toHaveBeenCalledWith("/files/7/download", {
			headers: {
				Range: "bytes=0-0",
			},
			responseType: "blob",
			validateStatus: expect.any(Function),
		});
	});

	it.each([
		"/s/share-token/download",
		"/api/v1/s/share-token/stream/session-token/video.mp4",
		"/d/direct-token/file.mp4",
		"/pv/preview-token/file.mp4",
		"https://cdn.example/file.mp4",
		"blob:http://localhost/file",
	])("skips public or already external resource %s", async (path) => {
		const { prepareAuthenticatedResource } = await import(
			"@/lib/authenticatedResource"
		);

		await prepareAuthenticatedResource(path);

		expect(mockState.ensureFreshSession).not.toHaveBeenCalled();
		expect(mockState.get).not.toHaveBeenCalled();
	});

	it("propagates auth failures from the probe", async () => {
		const error = { status: 401 };
		mockState.get.mockRejectedValue(error);
		const { prepareAuthenticatedResource } = await import(
			"@/lib/authenticatedResource"
		);

		await expect(
			prepareAuthenticatedResource("/files/7/download"),
		).rejects.toBe(error);
	});

	it("does not block native media loading for non-auth probe failures", async () => {
		const error = new Error("cors probe failed");
		mockState.get.mockRejectedValue(error);
		const { prepareAuthenticatedResource } = await import(
			"@/lib/authenticatedResource"
		);

		await expect(
			prepareAuthenticatedResource("/files/7/download"),
		).resolves.toBeUndefined();
		expect(mockState.debug).toHaveBeenCalledWith(
			"authenticated resource probe failed",
			"/files/7/download",
			error,
		);
	});
});
