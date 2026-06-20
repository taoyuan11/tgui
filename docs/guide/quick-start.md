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

仓库中的示例是 workspace member，可以用 package 名运行，也可以直接指定 manifest：

```sh
cargo run -p basic_window
cargo run --manifest-path examples/basic_window/Cargo.toml
cargo run --manifest-path examples/mvvm_counter/Cargo.toml
cargo run --manifest-path examples/canvas/Cargo.toml
```

更多示例见[示例索引](/advanced/examples)。

## 下一步

- 想配置标题、窗口大小、主题、资源预算和多窗口，读[应用与窗口](/guide/application)。
- 想理解状态如何驱动 UI，读[MVVM 状态模型](/guide/mvvm)。
- 想直接查组件用法，读[组件](/features/widgets)和[表单增强控件](/features/input-controls)。
- 想做自定义绘制或图表，读[Canvas](/features/canvas)。
- 想使用原生文件选择、系统通知或无边框窗口，读[对话框与通知](/features/dialogs-notifications)和[自定义窗口 Chrome](/features/window-chrome)。
