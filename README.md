# tgui

[](https://www.google.com/search?q=https://crates.io/crates/tgui)
[](https://www.google.com/search?q=https://docs.rs/tgui)
[](https://www.google.com/search?q=LICENSE)

**tgui** 是一个为 Rust 打造的、极速且类型安全的现代 GUI 库。它借鉴了声明式 UI 的灵活性，并结合了 Rust 原生的高性能渲染管线。

> **核心哲学：** 让复杂的界面开发像写脚本一样简单，同时保持系统级语言的运行效率。

-----

## ✨ 核心特性

* 🚀 **GPU 加速渲染：** 基于 `wgpu` 和 `Vello`，利用计算着色器（Compute Shaders）实现细腻的矢量绘图。
* 🦀 **完全 Rust 驱动：** 零不安全代码（Unsafe）愿景，深度利用借用检查器确保界面状态安全。
* ⚡ **细粒度响应式：** 采用信号（Signals）机制，仅在数据变动时更新受影响的组件，拒绝昂贵的全局 Diff。
* 📐 **现代布局系统：** 内置 Taffy 高性能布局引擎，原生支持 Flexbox 和 CSS Grid。
* 🎨 **开发者友好：** 支持热重载（Hot Reloading）和声明式宏，极大缩短 UI 迭代周期。
* 🌍 **多端一致性：** 无论在 Windows、macOS、Linux 还是 Web (WASM)，都能获得像素级一致的视觉体验。

-----

## 🏗️ 架构设计

**tgui** 采用了分层架构以确保灵活性和可维护性：

1.  **视图层 (View Layer):** 使用类似 JSX 的 Rust 声明式宏定义组件树。
2.  **反应式核心 (Reactive Core):** 基于信号的数据流管理，处理 `Model -> View` 的映射。
3.  **布局引擎 (Layout Engine):** 负责节点的大小、偏移和层级计算。
4.  **渲染抽象层 (Render Backend):** 将 UI 指令转换为 GPU 指令。

-----

## 🚀 快速上手

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
tgui = "0.1.0"
```

编写你的第一个窗口程序：

```rust
use tgui::prelude::*;

fn main() {
    tgui::run(app);
}

fn app() -> impl View {
    // 定义一个响应式状态
    let count = create_signal(0);

    column([
        text("Hello, tgui!").font_size(24),
        
        row([
            button(text("-")).on_click(move |_| count.update(|n| *n -= 1)),
            text(move || format!("Count: {}", count.get())),
            button(text("+")).on_click(move |_| count.update(|n| *n += 1)),
        ]).spacing(10),
    ])
    .center()
}
```

-----

## 🛠️ 技术栈说明

* **窗口管理:** [winit](https://github.com/rust-windowing/winit)
* **图形后端:** [wgpu](https://github.com/gfx-rs/wgpu)
* **矢量渲染:** [Vello](https://github.com/linebender/vello)
* **布局计算:** [Taffy](https://github.com/DioxusLabs/taffy)
* **文本排版:** [Cosmic-text](https://github.com/pop-os/cosmic-text)

-----

## 🤝 贡献指南

我们非常欢迎来自社区的贡献！无论是寻找 Bug、改进文档还是提交新特性，请先查阅我们的 [Contributing Guide](https://www.google.com/search?q=CONTRIBUTING.md)。

-----

## 📄 开源协议

**tgui** 采用 [MIT](https://www.google.com/search?q=LICENSE-MIT) 或 [Apache-2.0](https://www.google.com/search?q=LICENSE-APACHE) 双协议授权。

-----

**你想让我为你搭建一个支持上述 README 功能的最小可行性 (MVP) 项目结构吗？我可以为你写出基础的目录和核心 Trait 定义。**
