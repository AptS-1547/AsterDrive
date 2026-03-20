import { api } from './http'
import { endpoints } from './endpoints'
import type { TokenResponse, UserInfo } from '@/types/api'

export const authService = {
  login: (username: string, password: string) =>
    api.post<TokenResponse>(endpoints.auth.login, { username, password }),

  register: (username: string, email: string, password: string) =>
    api.post<UserInfo>(endpoints.auth.register, { username, email, password }),

  refresh: (refreshToken: string) =>
    api.post<{ access_token: string }>(endpoints.auth.refresh, {
      refresh_token: refreshToken,
    }),
}
