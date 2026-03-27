# tgui

`tgui` is a modern, GPU-accelerated Rust GUI framework built with `wgpu`, `cosmic-text`, and `taffy`.

It currently provides:
- A window/runtime layer based on `winit`
- MVVM-style state, bindings, and commands
- A widget tree with `Row`, `Column`, `Grid`, `Flex`, and `Stack`
- Text rendering with custom font registration and fallback

## Status

This project is under active development.

- The architecture is usable for demos and local tools.
- APIs may still change while milestones continue.

## Features

- `Application` builder with window/theme/font configuration
- `Observable<T>` and `Binding<T>` for reactive UI data flow
- `Command` and `ValueCommand` for UI-to-ViewModel actions
- `Text`, `Button`, `Input`, and layout container widgets
- `Taffy` as the layout engine
- `wgpu`-based rendering backend

## Installation

```toml
[dependencies]
tgui = "0.0.1"
```

or

```toml
[dependencies]
tgui = { git = "https://github.com/nandebishitaoyuan/tgui.git" }
```

## Quick Start

### 1. Basic Window

```rust
use tgui::Application;

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .title("tgui")
        .window_size(960, 640)
        .run()
}
```

### 2. MVVM + Widget Tree

```rust
use tgui::{Application, Binding, Column, Insets, Text, ViewModelContext};

struct DemoVm {
    count: tgui::Observable<u32>,
}

impl DemoVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            count: ctx.observable(0),
        }
    }

    fn title(&self) -> Binding<String> {
        self.count.binding().map(|n| format!("count: {n}"))
    }

    fn view(&self) -> tgui::Element<Self> {
        Column::new()
            .padding(Insets::all(24.0))
            .child(Text::new(self.count.binding().map(|n| format!("Clicks: {n}"))))
            .into()
    }
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .with_view_model(DemoVm::new)
        .bind_title(DemoVm::title)
        .root_view(DemoVm::view)
        .run()
}
```

## Fonts

Register custom fonts and optionally set a default font:

```rust
Application::new()
    .font("icon", include_bytes!("./assets/icon-font.ttf"))
    .default_font("JetBrains Mono");
```

Use a specific font on a text widget:

```rust
Text::new("Hello").font("JetBrains Mono")
```

## Run Examples

```bash
cargo run --example basic_window
cargo run --example mvvm_counter
cargo run --example widgets_showcase
cargo run --example layout_theme_showcase
```

## Development

```bash
cargo fmt
cargo check
cargo check --examples
```

## Current Limitations

- Text rendering is still evolving and may receive further performance improvements.
- APIs are not frozen yet and may change between milestones.

## License

Dual-licensed under:
- MIT
- Apache-2.0
