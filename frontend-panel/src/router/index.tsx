import { createBrowserRouter, Navigate, Outlet } from 'react-router-dom'
import { lazy, Suspense } from 'react'
import { useAuthStore } from '@/stores/authStore'

const LoginPage = lazy(() => import('@/pages/LoginPage'))
const FileBrowserPage = lazy(() => import('@/pages/FileBrowserPage'))

function Loading() {
  return (
    <div className="min-h-screen flex items-center justify-center">
      <div className="text-muted-foreground">Loading...</div>
    </div>
  )
}

function ProtectedRoute() {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated)
  const isChecking = useAuthStore((s) => s.isChecking)
  if (isChecking) return <Loading />
  if (!isAuthenticated) return <Navigate to="/login" replace />
  return (
    <Suspense fallback={<Loading />}>
      <Outlet />
    </Suspense>
  )
}

function LoginGuard() {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated)
  const isChecking = useAuthStore((s) => s.isChecking)
  if (isChecking) return <Loading />
  if (isAuthenticated) return <Navigate to="/" replace />
  return (
    <Suspense fallback={<Loading />}>
      <Outlet />
    </Suspense>
  )
}

export const router = createBrowserRouter([
  {
    element: <LoginGuard />,
    children: [{ path: '/login', element: <LoginPage /> }],
  },
  {
    element: <ProtectedRoute />,
    children: [{ path: '/', element: <FileBrowserPage /> }],
  },
])
