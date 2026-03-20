import { api } from './http'
import { endpoints } from './endpoints'
import type { UserInfo } from '@/types/api'

export const authService = {
  login: (username: string, password: string) =>
    api.post<null>(endpoints.auth.login, { username, password }),

  register: (username: string, email: string, password: string) =>
    api.post<UserInfo>(endpoints.auth.register, { username, email, password }),

  logout: () => api.post<null>('/auth/logout'),

  me: () => api.get<UserInfo>('/auth/me'),
}
