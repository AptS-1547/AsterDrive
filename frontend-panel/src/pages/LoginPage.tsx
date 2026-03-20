import { useState } from 'react'
import type { FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { useAuthStore } from '@/stores/authStore'
import { authService } from '@/services/authService'
import { handleApiError } from '@/hooks/useApiError'
import { toast } from 'sonner'

export default function LoginPage() {
  const [mode, setMode] = useState<'login' | 'register'>('login')
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [loading, setLoading] = useState(false)
  const login = useAuthStore((s) => s.login)
  const navigate = useNavigate()

  const resetForm = () => {
    setUsername('')
    setEmail('')
    setPassword('')
  }

  const handleLogin = async (e: FormEvent) => {
    e.preventDefault()
    setLoading(true)
    try {
      await login(username, password)
      navigate('/', { replace: true })
    } catch (error) {
      handleApiError(error)
    } finally {
      setLoading(false)
    }
  }

  const handleRegister = async (e: FormEvent) => {
    e.preventDefault()
    setLoading(true)
    try {
      await authService.register(username, email, password)
      toast.success('Account created! Signing in...')
      // 注册成功后自动登录
      await login(username, password)
      navigate('/', { replace: true })
    } catch (error) {
      handleApiError(error)
    } finally {
      setLoading(false)
    }
  }

  const switchMode = () => {
    resetForm()
    setMode(mode === 'login' ? 'register' : 'login')
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-background">
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle className="text-2xl text-center">AsterDrive</CardTitle>
          <CardDescription className="text-center">
            {mode === 'login' ? 'Sign in to your account' : 'Create a new account'}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={mode === 'login' ? handleLogin : handleRegister} className="space-y-4">
            <Input
              placeholder="Username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              required
              autoFocus
            />
            {mode === 'register' && (
              <Input
                type="email"
                placeholder="Email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
              />
            )}
            <Input
              type="password"
              placeholder="Password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
            />
            <Button type="submit" className="w-full" disabled={loading}>
              {loading
                ? (mode === 'login' ? 'Signing in...' : 'Creating account...')
                : (mode === 'login' ? 'Sign In' : 'Create Account')
              }
            </Button>
          </form>
          <div className="mt-4 text-center text-sm text-muted-foreground">
            {mode === 'login' ? "Don't have an account?" : 'Already have an account?'}{' '}
            <button
              type="button"
              className="text-primary underline-offset-4 hover:underline"
              onClick={switchMode}
            >
              {mode === 'login' ? 'Sign up' : 'Sign in'}
            </button>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
