import type { AxiosInstance } from "axios";
import axios from "axios";
import { config } from "@/config/app";
import {
	type ApiResponse,
	ErrorCode,
	type ErrorCode as ErrorCodeType,
} from "@/types/api-helpers";

const client: AxiosInstance = axios.create({
	baseURL: config.apiBaseUrl,
	timeout: 30000,
	headers: { "Content-Type": "application/json" },
	withCredentials: true,
});

// 不需要自动 refresh 的路径
const SKIP_REFRESH_PATHS = [
	"/auth/refresh",
	"/auth/login",
	"/auth/register",
	"/auth/logout",
	"/auth/check",
	"/auth/setup",
];

function shouldSkipRefresh(url: string) {
	if (SKIP_REFRESH_PATHS.some((path) => url.endsWith(path))) return true;
	return url.includes("/s/");
}

let isRefreshing = false;
let refreshPromise: Promise<void> | null = null;

client.interceptors.response.use(
	(res) => res,
	async (error) => {
		const original = error.config;
		const url = original?.url || "";

		// 跳过公开端点的自动 refresh（避免把分享页误当成登录态接口）
		const shouldSkip = shouldSkipRefresh(url);
		if (error.response?.status === 401 && !original._retry && !shouldSkip) {
			original._retry = true;

			if (isRefreshing) {
				await refreshPromise;
			} else {
				isRefreshing = true;
				refreshPromise = (async () => {
					const { useAuthStore } = await import("@/stores/authStore");
					await useAuthStore.getState().refreshToken();
				})().finally(() => {
					isRefreshing = false;
					refreshPromise = null;
				});
			}

			try {
				await refreshPromise;
				return client(original);
			} catch (refreshError) {
				// 网络错误（离线）时不强制登出
				if (!axios.isAxiosError(refreshError) || !refreshError.response) {
					return Promise.reject(error);
				}
				const { forceLogout } = await import("@/stores/authStore");
				forceLogout();
				window.location.href = "/login";
				return Promise.reject(error);
			}
		}
		return Promise.reject(error);
	},
);

export class ApiError extends Error {
	code: ErrorCodeType;
	constructor(code: ErrorCodeType, message: string) {
		super(message);
		this.code = code;
	}
}

async function unwrap<T>(
	promise: Promise<{ data: ApiResponse<T> }>,
): Promise<T> {
	const { data: resp } = await promise;
	if (resp.code !== ErrorCode.Success) {
		throw new ApiError(resp.code, resp.msg);
	}
	return resp.data as T;
}

export const api = {
	get: <T>(url: string, config?: { params?: object }) =>
		unwrap<T>(client.get(url, config)),
	post: <T>(url: string, data?: unknown) => unwrap<T>(client.post(url, data)),
	put: <T>(url: string, data?: unknown) => unwrap<T>(client.put(url, data)),
	patch: <T>(url: string, data?: unknown) => unwrap<T>(client.patch(url, data)),
	delete: <T>(url: string) => unwrap<T>(client.delete(url)),
	client,
};
