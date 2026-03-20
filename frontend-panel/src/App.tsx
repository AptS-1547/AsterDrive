import { RouterProvider } from 'react-router-dom'
import { Toaster } from 'sonner'
import { router } from '@/router'
import { useEffect } from 'react'
import { useAuthStore } from '@/stores/authStore'

function App() {
  const restore = useAuthStore((s) => s.restore)

  useEffect(() => {
    restore()
  }, [restore])

  return (
    <>
      <RouterProvider router={router} />
      <Toaster position="top-right" richColors />
    </>
  )
}

export default App
