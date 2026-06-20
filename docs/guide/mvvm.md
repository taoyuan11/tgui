# MVVM 状态模型

`tgui` 的公开启动路径是 MVVM。ViewModel 持有状态、暴露命令，并返回声明式组件树。组件树可以被频繁重建；真正需要保留的业务数据、文本编辑状态、动画控制器和异步任务句柄都应放在 ViewModel 中。

## 核心类型

| 类型 | 作用 | 常见位置 |
| --- | --- | --- |
| `ViewModel` | 应用状态和界面声明的宿主 | 为每个窗口或页面实现 |
| `ViewModelContext` | 创建 `State<T>`、`TextController`、动画值和 timeline | `ViewModel::new` |
| `State<T>` | 可写响应式状态，写入后触发 invalidation | ViewModel 字段 |
| `Signal<T>` | 惰性只读值，可 `map` 派生 UI 属性 | 传给组件属性 |
| `TextController` | 保留式文本状态，包含文本修订和输入 invalidation | `Input` / `Textarea` |
| `Command<VM>` | 无 payload 事件处理 | `on_click`、`on_mount` |
| `ValueCommand<VM, V>` | 带 payload 事件处理 | `on_change`、`on_open_change` |
| `CommandContext` | 命令中的运行时服务入口 | 对话框、通知、窗口控制、日志 |

## 最小 ViewModel

```rust
use tgui::prelude::*;

struct CounterVm {
    count: State<i32>,
}

impl CounterVm {
    fn increment(&mut self) {
        self.count.update(|value| *value += 1);
    }
}

impl ViewModel for CounterVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            count: ctx.state(0),
        }
    }

    fn view(&self) -> Element<Self> {
        Flex::vertical().gap(dp(12.0)).child(el![
            Text::new(self.count.signal().map(|count| format!("Count: {count}"))),
            Button::new("Increment").on_click(Command::new(Self::increment)),
        ]).into()
    }
}
```

关键点：

- `State<T>` 放在 ViewModel 字段里。
- UI 读取状态时用 `state.signal()`，再用 `map` 派生字符串、颜色、尺寸等展示值。
- 事件通过 `Command` 回到 `&mut ViewModel`，而不是让 widget 直接修改外部变量。

## State 与 Signal

`State<T>` 负责写入，`Signal<T>` 负责传给 UI。

```rust
fn title(&self) -> Signal<String> {
    self.count
        .signal()
        .map(|count| format!("tgui counter - {count}"))
}

fn clear_color(&self) -> Signal<Color> {
    self.count.signal().map(|count| {
        if count % 2 == 0 {
            Color::hexa(0x0F172AFF)
        } else {
            Color::hexa(0x10253CFF)
        }
    })
}
```

常用 `State` API：

| API | 说明 |
| --- | --- |
| `ctx.state(initial)` | 创建状态。 |
| `state.get()` | 克隆当前值。适合小对象和命令处理。 |
| `state.read(|value| ...)` / `with_ref` | 借用读取，避免克隆大对象。 |
| `state.set(value)` | 值变化时触发刷新，需要 `T: PartialEq`。 |
| `state.update(|value| ...)` | 原地修改，值变化时触发刷新，需要 `T: Clone + PartialEq`。 |
| `state.mutate(|value| ...)` | 原地修改并无条件触发刷新。 |
| `state.signal()` | 创建惰性只读信号。 |
| `state.project(|value| ...)` | 借用式派生信号，适合从大对象取字段。 |

常用 `Signal` API：

| API | 说明 |
| --- | --- |
| `signal.get()` | 读取当前值。 |
| `signal.map(...)` | 派生新信号。 |
| `signal.map_memo(...)` | 派生并按 `PartialEq` 缓存，减少无效刷新。 |
| `signal.project(...)` | 借用式派生，减少克隆。 |
| `signal.animated(Transition)` | 给支持插值的属性添加声明式过渡。 |

## 受控组件

控件通常读取 `Signal`，在变化事件里写回对应 `State`。

```rust
struct SettingsVm {
    dark_mode: State<bool>,
    volume: State<f32>,
}

impl SettingsVm {
    fn set_dark_mode(&mut self, enabled: bool) {
        self.dark_mode.set(enabled);
    }

    fn set_volume(&mut self, value: f32) {
        self.volume.set(value);
    }
}

Flex::vertical().gap(dp(12.0)).child(el![
    Switch::new(self.dark_mode.signal())
        .on_change(ValueCommand::new(SettingsVm::set_dark_mode)),
    Slider::new(self.volume.signal(), 0.0, 100.0)
        .step(5.0)
        .show_value_label(true)
        .on_change(ValueCommand::new(SettingsVm::set_volume)),
])
```

`ValueCommand` 的 payload 类型由组件决定：`Switch` 是 `bool`，`Slider` 是 `f32`，`Select` 是 `(K, V)`，`DatePicker` 是 `DatePickerChange`。

## Command 与 CommandContext

无 payload 的事件使用 `Command`：

```rust
Button::new("刷新")
    .on_click(Command::new(|vm: &mut AppVm| {
        vm.reload();
    }))
```

需要访问运行时服务时使用 `new_with_context`：

```rust
Button::new("发送通知")
    .on_click(Command::new_with_context(|vm: &mut AppVm, ctx| {
        ctx.log().info("notification requested");
        let result = ctx.notifications().send(
            NotificationOptions::new("完成").body("后台任务已经结束"),
        );
        if let Err(error) = result {
            vm.status.set(format!("通知发送失败: {error}"));
        }
    }))
```

`CommandContext` 常用服务：

| API | 说明 |
| --- | --- |
| `ctx.window()` | 拖拽窗口、调整大小、最小化、最大化、关闭等。 |
| `ctx.dialogs()` | 原生文件选择和消息框。 |
| `ctx.notifications()` | 系统通知和通知 action 回调。 |
| `ctx.log()` | 写入 tgui 日志。 |

## 文本输入

`Input` 和 `Textarea` 使用 `TextController` 保存文本、修订号和 invalidation。真实输入场景优先使用控制器，而不是单独维护 `State<String>`。

```rust
struct ProfileVm {
    name: TextController,
    bio: TextController,
    status: State<String>,
}

impl ViewModel for ProfileVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            name: ctx.text_controller(""),
            bio: ctx.text_controller(""),
            status: ctx.state(String::new()),
        }
    }

    fn view(&self) -> Element<Self> {
        Flex::vertical().gap(dp(10.0)).child(el![
            Input::new(self.name.clone())
                .placeholder("姓名")
                .on_change(Command::new(|vm: &mut ProfileVm| {
                    vm.status.set(format!("name: {}", vm.name.text()));
                })),
            Textarea::new(self.bio.clone())
                .height(dp(120.0))
                .placeholder("个人简介")
                .on_change_set(ValueCommand::new(|vm: &mut ProfileVm, change: TextChangeSet| {
                    vm.status.set(format!("文本修订: {}", change.end_revision));
                })),
            Text::new(self.status.signal()),
        ]).into()
    }
}
```

常用 `TextController` API：

| API | 说明 |
| --- | --- |
| `ctx.text_controller(initial)` | 创建控制器。 |
| `controller.text()` | 读取当前文本。 |
| `controller.with_text(|text| ...)` | 借用读取。 |
| `controller.snapshot()` | 获取文本和修订号。 |
| `controller.set_text(...)` / `replace_all(...)` | 主动替换文本并触发刷新。 |

## 表单和校验

`Form`、`FormField<T>` 和 `TextFormField` 位于 ViewModel 层，用于聚合值、校验、提交和重置。它们不绑定具体视觉布局，适合复用在不同页面。

```rust
Input::new(self.profile_email.controller())
    .placeholder("name@example.com")
    .validation(self.profile_email.validation_state())

Checkbox::new(self.profile_newsletter.signal())
    .label("订阅每周邮件")
    .validation(self.profile_newsletter.validation_state())
    .on_change(self.profile_newsletter.bind_change())
```

提交时可以把快照交给业务逻辑：

```rust
let command = self.profile_form.submit_async_command(
    ValueCommand::new(|vm: &mut AppVm, snapshot: FormSnapshot| {
        let name = snapshot.get::<String>("name").unwrap_or_default();
        vm.status.set(format!("已提交: {name}"));
    }),
);
```

## 多 ViewModel

大型应用可以拆成多个 ViewModel，再用 `Element::scope(...)` 把子树命令映射到子 ViewModel。

```rust
struct RootVm {
    settings: SettingsVm,
}

fn view(&self) -> Element<Self> {
    SettingsVm::view(&self.settings)
        .scope(|root: &mut RootVm| &mut root.settings)
}
```

这种模式适合复杂页面拆分、设置面板、详情页和多个独立业务模块。子 ViewModel 仍然遵守同样的状态和命令规则。

## 经验规则

- ViewModel 字段保存业务状态、控制器、动画句柄和队列；不要把这些数据藏在 widget 构造闭包里。
- UI 读取使用 `Signal`，事件写回使用 `Command` / `ValueCommand`。
- 大对象派生值优先用 `State::project` 或 `Signal::project`，减少克隆。
- 文本输入优先用 `TextController`，它负责文本修订、IME 和刷新基础设施。
- 需要窗口、通知、对话框等运行时能力时，在命令里使用 `CommandContext`。
- `view()` 可以便宜地重建；避免在 `view()` 里做阻塞 IO、网络请求或昂贵计算。
