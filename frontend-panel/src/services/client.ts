import createClient from 'openapi-fetch'
import type { paths } from './api.generated'
import { config } from '@/config/app'

export const apiClient = createClient<paths>({
  baseUrl: config.apiBaseUrl.replace('/api/v1', ''),
  credentials: 'include',
})
