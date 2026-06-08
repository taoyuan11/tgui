import { defineConfig } from 'vitepress'

export default defineConfig({
  lang: 'zh-CN',
  title: 'tgui',
  description: 'A modern, GPU-accelerated Rust GUI framework with MVVM, Taffy layout, and wgpu rendering.',
  cleanUrls: true,
  lastUpdated: true,
  head: [
    ['link', { rel: 'icon', href: '/images/tgui_logo.png' }]
  ],
  themeConfig: {
    logo: '/images/tgui_logo.png',
    siteTitle: 'tgui',
    search: {
      provider: 'local'
    },
    nav: [
      { text: '指南', link: '/guide/quick-start' },
      { text: '核心能力', link: '/features/widgets' },
      { text: '深入', link: '/advanced/runtime' },
      { text: '示例', link: '/advanced/examples' }
    ],
    sidebar: [
      {
        text: '指南',
        items: [
          { text: '快速开始', link: '/guide/quick-start' },
          { text: '应用与窗口', link: '/guide/application' },
          { text: 'MVVM 状态模型', link: '/guide/mvvm' }
        ]
      },
      {
        text: '核心能力',
        items: [
          { text: '布局系统', link: '/features/layout' },
          { text: '组件', link: '/features/widgets' },
          { text: '表单增强控件', link: '/features/input-controls' },
          { text: 'P3 体验组件', link: '/features/p3-components' },
          { text: 'Canvas', link: '/features/canvas' },
          { text: '主题与样式', link: '/features/theme' },
          { text: '媒体', link: '/features/media' },
          { text: '对话框与通知', link: '/features/dialogs-notifications' },
          { text: '自定义窗口 Chrome', link: '/features/window-chrome' }
        ]
      },
      {
        text: '深入',
        items: [
          { text: '运行时与渲染', link: '/advanced/runtime' },
          { text: '性能与资源', link: '/advanced/performance' },
          { text: '示例索引', link: '/advanced/examples' }
        ]
      },
      {
        text: '迁移',
        items: [
          { text: 'Theme and Style API v2', link: '/migration/theme-style-v2' }
        ]
      }
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/nandebishitaoyuan/tgui' }
    ],
    footer: {
      message: 'Released under the Apache-2.0 license.',
      copyright: 'Copyright © tgui contributors'
    }
  }
})
