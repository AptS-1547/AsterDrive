import axios from 'axios'
import type { AxiosInstance } from 'axios'
import { config } from '@/config/app'
import type { ApiResponse } from '@/types/api'
import { ErrorCode } from '@/types/api'

const client: AxiosInstance = axios.create({
  baseURL: config.apiBaseUrl,
  timeout: 30000,
  headers: { 'Content-Type': 'application/json' },
})

// Request interceptor — inject Bearer token
client.interceptors.request.use((req) => {
  const token = localStorage.getItem('access_token')
  if (token && req.headers) {
    req.headers.Authorization = `Bearer ${token}`
  }
  return req
})

// Response interceptor — handle 401 with token refresh
let isRefreshing = false
let refreshQueue: Array<(token: string) => void> = []

client.interceptors.response.use(
  (res) => res,
  async (error) => {
    const original = error.config
    if (error.response?.status === 401 && !original._retry) {
      original._retry = true
      const refreshToken = localStorage.getItem('refresh_token')

      if (!refreshToken) {
        localStorage.clear()
        window.location.href = '/login'
        return Promise.reject(error)
      }

      if (isRefreshing) {
        return new Promise((resolve) => {
          refreshQueue.push((token: string) => {
            original.headers.Authorization = `Bearer ${token}`
            resolve(client(original))
          })
        })
      }

      isRefreshing = true
      try {
        const res = await axios.post(`${config.apiBaseUrl}/auth/refresh`, {
          refresh_token: refreshToken,
        })
        const newToken = res.data.data.access_token
        localStorage.setItem('access_token', newToken)
        refreshQueue.forEach((cb) => cb(newToken))
        refreshQueue = []
        original.headers.Authorization = `Bearer ${newToken}`
        return client(original)
      } catch {
        localStorage.clear()
        window.location.href = '/login'
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

// Unwrap ApiResponse, throw ApiError on non-zero code
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
  // Raw client for multipart etc.
  client,
}
