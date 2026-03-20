export const endpoints = {
  auth: {
    register: '/auth/register',
    login: '/auth/login',
    refresh: '/auth/refresh',
  },
  files: {
    upload: '/files/upload',
    get: (id: number) => `/files/${id}`,
    download: (id: number) => `/files/${id}/download`,
    delete: (id: number) => `/files/${id}`,
    update: (id: number) => `/files/${id}`,
  },
  folders: {
    root: '/folders',
    get: (id: number) => `/folders/${id}`,
    create: '/folders',
    delete: (id: number) => `/folders/${id}`,
    update: (id: number) => `/folders/${id}`,
  },
} as const
