# 快速开始

`tgui` 是一个桌面端 Rust GUI crate。应用入口是 `Application`，界面由 ViewModel 返回的组件树描述，状态通过 `State<T>` 和 `Signal<T>` 驱动刷新。

## 安装

```toml
[dependencies]
tgui = "0.2.0"
```

启用音频或视频能力时使用 feature：

```toml
[dependencies]
tgui = { version = "0.2.0", features = ["audio"] }
tgui = { version = "0.2.0", features = ["video"] }
```

当前移动端入口未开放，主要目标平台是 Windows、macOS 和 Linux 桌面端。

## 最小计数器

```rust
use tgui::prelude::*;

struct CounterVm {
    count: State<u32>,
}

impl CounterVm {
    fn increment(&mut self) {
        self.count.update(|value| *value += 1);
    }

    fn view_content(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .child(Text::new(
                self.count.signal().map(|count| format!("Count: {count}")),
            ))
            .child(Button::new("Increment").on_click(Command::new(Self::increment)))
            .into()
    }
}

impl ViewModel for CounterVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            count: ctx.state(0),
        }
    }

    fn view(&self) -> Element<Self> {
        self.view_content()
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .title("Counter")
        .window_size(dp(360.0), dp(240.0))
        .with_view_model(CounterVm::new)
        .root_view(CounterVm::view)
        .run()
}
```

## 运行示例

仓库中的示例是独立 Cargo 工程，可以直接指定 manifest 运行：

```sh
cargo run --manifest-path examples/basic_window/Cargo.toml
cargo run --manifest-path examples/mvvm_counter/Cargo.toml
cargo run --manifest-path examples/canvas/Cargo.toml
```

更多示例见[示例索引](/advanced/examples)。
