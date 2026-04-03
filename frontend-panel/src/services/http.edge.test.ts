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

	return {
		axiosModule,
		client,
		forceLogout: vi.fn(),
		getErrorHandler: () => {
			if (!errorHandler)
				throw new Error("response error handler not registered");
			return errorHandler;
		},
		logout: vi.fn(async () => undefined),
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

describe("http refresh edge cases", () => {
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
		Object.defineProperty(window, "location", {
			configurable: true,
			value: {
				href: "http://localhost/",
			},
		});
	});

	it("keeps the original error when refresh fails due to a network problem", async () => {
		const refreshError = new Error("offline");
		mockState.refreshToken.mockRejectedValue(refreshError);
		mockState.axiosModule.isAxiosError.mockReturnValue(false);
		await loadHttpModule();
		const errorHandler = mockState.getErrorHandler();
		const originalError = {
			config: { url: "/files", _retry: false },
			response: { status: 401 },
		} satisfies MockAxiosError;

		await expect(errorHandler(originalError)).rejects.toBe(originalError);
		expect(mockState.forceLogout).not.toHaveBeenCalled();
		expect(window.location.href).toBe("http://localhost/");
	});

	it("forces logout when refresh fails with an auth response", async () => {
		mockState.axiosModule.isAxiosError.mockReturnValue(true);
		mockState.refreshToken.mockRejectedValue({
			isAxiosError: true,
			response: { status: 401 },
		});
		await loadHttpModule();
		const errorHandler = mockState.getErrorHandler();
		const originalError = {
			config: { url: "/files", _retry: false },
			response: { status: 401 },
		} satisfies MockAxiosError;

		await expect(errorHandler(originalError)).rejects.toBe(originalError);
		expect(mockState.forceLogout).toHaveBeenCalledTimes(1);
		expect(window.location.href).toBe("/login");
	});

	it("queues concurrent 401 retries behind a single refresh call", async () => {
		let resolveRefresh: (() => void) | undefined;
		mockState.refreshToken.mockReturnValue(
			new Promise((resolve) => {
				resolveRefresh = () => resolve({});
			}),
		);
		mockState.client.mockResolvedValue({
			data: {
				code: ErrorCode.Success,
				msg: "ok",
				data: { retried: true },
			},
		});
		await loadHttpModule();
		const errorHandler = mockState.getErrorHandler();

		const first = errorHandler({
			config: { url: "/files/1", _retry: false },
			response: { status: 401 },
		});
		const second = errorHandler({
			config: { url: "/files/2", _retry: false },
			response: { status: 401 },
		});

		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(mockState.refreshToken).toHaveBeenCalledTimes(1);

		resolveRefresh?.();

		await expect(Promise.all([first, second])).resolves.toEqual([
			expect.objectContaining({
				data: expect.objectContaining({
					data: { retried: true },
				}),
			}),
			expect.objectContaining({
				data: expect.objectContaining({
					data: { retried: true },
				}),
			}),
		]);
		expect(mockState.client).toHaveBeenCalledTimes(2);
	});
});
