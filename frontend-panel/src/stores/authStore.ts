import { create } from 'zustand'
import { authService } from '@/services/authService'

interface AuthState {
  isAuthenticated: boolean
  isChecking: boolean
  login: (username: string, password: string) => Promise<void>
  logout: () => Promise<void>
  checkAuth: () => Promise<void>
}

export const useAuthStore = create<AuthState>((set) => ({
  isAuthenticated: false,
  isChecking: true,

  login: async (username, password) => {
    await authService.login(username, password)
    // login 成功后 cookie 已设置，标记为已认证
    set({ isAuthenticated: true })
  },

  logout: async () => {
    try {
      await authService.logout()
    } catch {
      // logout 失败不阻塞
    }
    set({ isAuthenticated: false })
  },

  checkAuth: async () => {
    set({ isChecking: true })
    try {
      await authService.me()
      set({ isAuthenticated: true, isChecking: false })
    } catch {
      set({ isAuthenticated: false, isChecking: false })
    }
  },
}))
