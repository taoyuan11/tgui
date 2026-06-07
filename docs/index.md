---
layout: home

hero:
  name: tgui
  text: GPU 加速的 Rust 桌面 GUI 框架
  tagline: 基于 wgpu、MVVM、taffy 布局和声明式组件树构建原生桌面应用。
  image:
    src: /images/tgui_logo.png
    alt: tgui logo
  actions:
    - theme: brand
      text: 快速开始
      link: /guide/quick-start
    - theme: alt
      text: 浏览组件
      link: /features/widgets

features:
  - title: MVVM 状态模型
    details: 使用 ViewModel、State、Signal 和 Command 把状态、派发和界面声明组织在同一条清晰链路里。
  - title: GPU 加速渲染
    details: 渲染管线基于 wgpu，覆盖文字、矩形、渐变、图片、Canvas mesh 与透明窗口等常见桌面 UI 需求。
  - title: Taffy 布局
    details: Flex、Grid、Stack、ScrollView 和虚拟列表都落在统一布局系统上，适合工具型和数据密集型界面。
  - title: 桌面能力
    details: 内置系统通知、原生对话框、自定义窗口 chrome、图片/SVG 加载，以及可选音频和视频播放能力。
---

## 适用场景

`tgui` 适合桌面 GUI、内部工具、可视化面板、自定义绘制界面，以及希望在 Rust 中保留清晰状态模型的应用。

当前版本仍处于 `0.x` 阶段，公共 API 可能继续调整。用于长期维护项目时建议固定 crate 版本，并在升级前阅读变更记录与迁移文档。
