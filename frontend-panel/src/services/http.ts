import axios from 'axios'
import type { AxiosInstance } from 'axios'
import { config } from '@/config/app'
import type { ApiResponse } from '@/types/api'
import { ErrorCode } from '@/types/api'

const client: AxiosInstance = axios.create({
  baseURL: config.apiBaseUrl,
  timeout: 30000,
  headers: { 'Content-Type': 'application/json' },
  withCredentials: true,
})

// 不需要自动 refresh 的路径
const SKIP_REFRESH_PATHS = ['/auth/me', '/auth/refresh', '/auth/login', '/auth/register', '/auth/logout']

let isRefreshing = false
let refreshQueue: Array<() => void> = []

client.interceptors.response.use(
  (res) => res,
  async (error) => {
    const original = error.config
    const url = original?.url || ''

    // 跳过 auth 端点的自动 refresh（避免死循环）
    const shouldSkip = SKIP_REFRESH_PATHS.some((p) => url.endsWith(p))
    if (error.response?.status === 401 && !original._retry && !shouldSkip) {
      original._retry = true

      if (isRefreshing) {
        return new Promise((resolve) => {
          refreshQueue.push(() => resolve(client(original)))
        })
      }

      isRefreshing = true
      try {
        await axios.post(`${config.apiBaseUrl}/auth/refresh`, null, {
          withCredentials: true,
        })
        refreshQueue.forEach((cb) => cb())
        refreshQueue = []
        return client(original)
      } catch {
        refreshQueue = []
        return Promise.reject(error)
      } finally {
        isRefreshing = false
      }
    }
    return Promise.reject(error)
  },
)

export class ApiError extends Error {
  code: ErrorCode
  constructor(code: ErrorCode, message: string) {
    super(message)
    this.code = code
  }
}

async function unwrap<T>(promise: Promise<{ data: ApiResponse<T> }>): Promise<T> {
  const { data: resp } = await promise
  if (resp.code !== ErrorCode.Success) {
    throw new ApiError(resp.code, resp.msg)
  }
  return resp.data as T
}

export const api = {
  get: <T>(url: string) => unwrap<T>(client.get(url)),
  post: <T>(url: string, data?: unknown) => unwrap<T>(client.post(url, data)),
  patch: <T>(url: string, data?: unknown) => unwrap<T>(client.patch(url, data)),
  delete: <T>(url: string) => unwrap<T>(client.delete(url)),
  client,
}
