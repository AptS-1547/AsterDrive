import { create } from 'zustand'
import { authService } from '@/services/authService'

interface AuthState {
  accessToken: string | null
  refreshToken: string | null
  isAuthenticated: boolean
  login: (username: string, password: string) => Promise<void>
  logout: () => void
  restore: () => void
}

export const useAuthStore = create<AuthState>((set) => ({
  accessToken: null,
  refreshToken: null,
  isAuthenticated: false,

  login: async (username, password) => {
    const tokens = await authService.login(username, password)
    localStorage.setItem('access_token', tokens.access_token)
    localStorage.setItem('refresh_token', tokens.refresh_token)
    set({
      accessToken: tokens.access_token,
      refreshToken: tokens.refresh_token,
      isAuthenticated: true,
    })
  },

  logout: () => {
    localStorage.removeItem('access_token')
    localStorage.removeItem('refresh_token')
    set({ accessToken: null, refreshToken: null, isAuthenticated: false })
  },

  restore: () => {
    const accessToken = localStorage.getItem('access_token')
    const refreshToken = localStorage.getItem('refresh_token')
    if (accessToken && refreshToken) {
      set({ accessToken, refreshToken, isAuthenticated: true })
    }
  },
}))
