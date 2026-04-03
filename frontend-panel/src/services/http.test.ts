import { beforeEach, describe, expect, it, vi } from "vitest";
import { ErrorCode } from "@/types/api-helpers";

type MockAxiosError = {
	config?: { _retry?: boolean; url?: string };
	isAxiosError?: boolean;
	response?: { status: number };
};

const mockState = vi.hoisted(() => {
	let errorHandler: ((error: MockAxiosError) => Promise<unknown>) | undefined;

	const client = vi.fn();
	client.get = vi.fn();
	client.post = vi.fn();
	client.put = vi.fn();
	client.patch = vi.fn();
	client.delete = vi.fn();
	client.interceptors = {
		response: {
			use: vi.fn(
				(
					_success: unknown,
					failure: (error: MockAxiosError) => Promise<unknown>,
				) => {
					errorHandler = failure;
					return 0;
				},
			),
		},
	};

	const axiosModule = {
		create: vi.fn(() => client),
		post: vi.fn(),
		isAxiosError: vi.fn(
			(error: unknown) => !!(error as MockAxiosError | undefined)?.isAxiosError,
		),
	};

	const logout = vi.fn(async () => undefined);

	return {
		axiosModule,
		client,
		forceLogout: vi.fn(),
		getErrorHandler: () => {
			if (!errorHandler)
				throw new Error("response error handler not registered");
			return errorHandler;
		},
		logout,
		refreshToken: vi.fn(async () => undefined),
	};
});

vi.mock("axios", () => ({
	default: mockState.axiosModule,
}));

vi.mock("@/stores/authStore", () => ({
	forceLogout: mockState.forceLogout,
	useAuthStore: {
		getState: () => ({
			logout: mockState.logout,
			refreshToken: mockState.refreshToken,
		}),
	},
}));

async function loadHttpModule() {
	vi.resetModules();
	return await import("@/services/http");
}

describe("http api helpers", () => {
	beforeEach(() => {
		mockState.axiosModule.create.mockClear();
		mockState.axiosModule.isAxiosError.mockClear();
		mockState.axiosModule.post.mockReset();
		mockState.client.mockReset();
		mockState.client.delete.mockReset();
		mockState.client.get.mockReset();
		mockState.client.patch.mockReset();
		mockState.client.post.mockReset();
		mockState.client.put.mockReset();
		mockState.client.interceptors.response.use.mockClear();
		mockState.forceLogout.mockClear();
		mockState.logout.mockClear();
		mockState.refreshToken.mockReset();
		mockState.refreshToken.mockResolvedValue(undefined);
	});

	it("unwraps successful responses from api.get", async () => {
		mockState.client.get.mockResolvedValue({
			data: {
				code: ErrorCode.Success,
				msg: "ok",
				data: { id: 7 },
			},
		});

		const { api } = await loadHttpModule();

		await expect(api.get("/files", { params: { limit: 10 } })).resolves.toEqual(
			{
				id: 7,
			},
		);
		expect(mockState.client.get).toHaveBeenCalledWith("/files", {
			params: { limit: 10 },
		});
	});

	it("throws ApiError when the backend response code is not success", async () => {
		mockState.client.get.mockResolvedValue({
			data: {
				code: ErrorCode.Forbidden,
				msg: "forbidden",
				data: null,
			},
		});

		const { ApiError, api } = await loadHttpModule();

		await expect(api.get("/files")).rejects.toEqual(
			expect.objectContaining({
				code: ErrorCode.Forbidden,
				message: "forbidden",
			}),
		);
		await expect(api.get("/files")).rejects.toBeInstanceOf(ApiError);
	});

	it("refreshes and retries a protected request after a 401", async () => {
		mockState.client.mockResolvedValue({
			data: {
				code: ErrorCode.Success,
				msg: "ok",
				data: { retried: true },
			},
		});

		await loadHttpModule();
		const errorHandler = mockState.getErrorHandler();
		const originalRequest = { url: "/files", _retry: false };

		await expect(
			errorHandler({
				config: originalRequest,
				response: { status: 401 },
			}),
		).resolves.toEqual({
			data: {
				code: ErrorCode.Success,
				msg: "ok",
				data: { retried: true },
			},
		});
		expect(mockState.refreshToken).toHaveBeenCalledTimes(1);
		expect(mockState.client).toHaveBeenCalledWith(
			expect.objectContaining({
				url: "/files",
				_retry: true,
			}),
		);
	});

	it("does not attempt refresh for public share endpoints", async () => {
		await loadHttpModule();
		const errorHandler = mockState.getErrorHandler();
		const originalError = {
			config: { url: "/s/token-1/content" },
			response: { status: 401 },
		} satisfies MockAxiosError;

		await expect(errorHandler(originalError)).rejects.toBe(originalError);
		expect(mockState.axiosModule.post).not.toHaveBeenCalled();
		expect(mockState.client).not.toHaveBeenCalled();
	});
});
