import { readFileSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vitepress'

const __dirname = dirname(fileURLToPath(import.meta.url))

function getVersion(): string {
  try {
    const cargoPath = resolve(__dirname, '../../Cargo.toml')
    const content = readFileSync(cargoPath, 'utf-8')
    const match = content.match(/^version\s*=\s*"([^"]+)"/m)
    return match ? match[1] : 'unknown'
  } catch {
    return 'unknown'
  }
}

const VERSION = getVersion()

export default defineConfig({
  title: 'AsterDrive',
  description: '自托管云存储系统，支持多存储策略、三种上传模式、分享、WebDAV、版本历史与回收站',

  locales: {
    root: {
      label: '简体中文',
      lang: 'zh-CN',
      themeConfig: {
        nav: [
          { text: '首页', link: '/' },
          { text: '快速开始', link: '/guide/getting-started' },
          { text: '使用指南', link: '/guide/user-guide' },
          { text: '配置', link: '/config/' },
          { text: 'API', link: '/api/' },
          { text: '部署', link: '/deployment/' },
          { text: '架构', link: '/architecture' },
          {
            text: `v${VERSION}`,
            items: [
              { text: '更新日志', link: 'https://github.com/AptS-1547/AsterDrive/releases' },
              { text: 'GitHub', link: 'https://github.com/AptS-1547/AsterDrive' }
            ]
          }
        ],
        footer: {
          message: '基于 MIT 许可证发布',
          copyright: 'Copyright © 2026 AptS:1547'
        },
        docFooter: { prev: '上一页', next: '下一页' },
        outline: { label: '页面导航' },
        lastUpdated: {
          text: '最后更新于',
          formatOptions: { dateStyle: 'short', timeStyle: 'medium' }
        },
        returnToTopLabel: '回到顶部',
        sidebarMenuLabel: '菜单',
        darkModeSwitchLabel: '主题',
        lightModeSwitchTitle: '切换到浅色模式',
        darkModeSwitchTitle: '切换到深色模式'
      }
    }
  },

  head: [
    ['meta', { name: 'theme-color', content: '#0F172A' }],
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' }],
    ['meta', { name: 'og:type', content: 'website' }],
    ['meta', { name: 'og:locale', content: 'zh_CN' }],
    ['meta', { name: 'og:title', content: 'AsterDrive | 自托管云存储' }],
    ['meta', { name: 'og:site_name', content: 'AsterDrive' }]
  ],

  themeConfig: {
    logo: '/logo.svg',
    siteTitle: false,

    sidebar: {
      '/guide/': [
        {
          text: '开始使用',
          items: [
            { text: '安装', link: '/guide/installation' },
            { text: '快速开始', link: '/guide/getting-started' },
            { text: '用户手册', link: '/guide/user-guide' },
            { text: '核心流程', link: '/guide/core-workflows' },
            { text: '上传模式', link: '/guide/upload-modes' },
            { text: '分享', link: '/guide/sharing' },
            { text: '文件编辑', link: '/guide/editing' },
            { text: '管理面板', link: '/guide/admin-console' }
          ]
        }
      ],
      '/config/': [
        {
          text: '配置',
          items: [
            { text: '配置概览', link: '/config/' },
            { text: '服务器', link: '/config/server' },
            { text: '数据库', link: '/config/database' },
            { text: '认证', link: '/config/auth' },
            { text: '存储策略', link: '/config/storage' },
            { text: 'WebDAV', link: '/config/webdav' },
            { text: '运行时配置', link: '/config/runtime' },
            { text: '缓存', link: '/config/cache' },
            { text: '日志', link: '/config/logging' }
          ]
        }
      ],
      '/api/': [
        {
          text: 'API 文档',
          items: [
            { text: 'API 概览', link: '/api/' },
            { text: '认证', link: '/api/auth' },
            { text: '文件', link: '/api/files' },
            { text: '文件夹', link: '/api/folders' },
            { text: '批量操作', link: '/api/batch' },
            { text: '分享', link: '/api/shares' },
            { text: '回收站', link: '/api/trash' },
            { text: 'WebDAV', link: '/api/webdav' },
            { text: '属性', link: '/api/properties' },
            { text: '管理', link: '/api/admin' },
            { text: '健康检查', link: '/api/health' }
          ]
        }
      ],
      '/deployment/': [
        {
          text: '部署',
          items: [
            { text: '部署概览', link: '/deployment/' },
            { text: '运行时行为', link: '/deployment/runtime-behavior' },
            { text: '前端资源', link: '/deployment/frontend-assets' },
            { text: 'Docker', link: '/deployment/docker' },
            { text: 'systemd', link: '/deployment/systemd' },
            { text: '反向代理', link: '/deployment/proxy' }
          ]
        }
      ],
      '/architecture': [
        {
          text: '架构',
          items: [{ text: '系统架构', link: '/architecture' }]
        }
      ]
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/AptS-1547/AsterDrive' }
    ],

    search: {
      provider: 'local',
      options: {
        locales: {
          zh: {
            translations: {
              button: { buttonText: '搜索文档', buttonAriaLabel: '搜索文档' },
              modal: {
                noResultsText: '无法找到相关结果',
                resetButtonTitle: '清除查询条件',
                footer: { selectText: '选择', navigateText: '切换' }
              }
            }
          }
        }
      }
    },

    editLink: {
      pattern: 'https://github.com/AptS-1547/AsterDrive/edit/master/docs/:path',
      text: '编辑此页面'
    }
  },

  markdown: {
    theme: { light: 'vitesse-light', dark: 'vitesse-dark' }
  }
})
