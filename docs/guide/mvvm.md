# MVVM 状态模型

`tgui` 的公开启动路径是 MVVM。ViewModel 持有状态、暴露命令，并返回声明式组件树。

## 核心类型

- `ViewModel`：应用状态和界面声明的宿主。
- `ViewModelContext`：创建 `State<T>`、`TextController`、动画值和时间线控制器。
- `State<T>`：可变状态，写入后触发 invalidation。
- `Signal<T>`：惰性读取值，可通过 `map` 派生 UI 属性。
- `Command<T>`：无 payload 的事件处理。
- `ValueCommand<T, V>`：带 payload 的事件处理。
- `TextController`：`Input` 和 `Textarea` 的保留式文本状态。

## State 与 Signal

`State<T>` 用于 ViewModel 内部可变数据。读取 UI 值时通常先转成 `Signal<T>`：

```rust
Text::new(self.count.signal().map(|count| format!("Count: {count}")))
```

当 `State::set` 或 `State::update` 改变值后，运行时会标记需要刷新，并在下一轮事件循环同步组件树。

## Command

组件事件通过 `Command` 回到 ViewModel：

```rust
Button::new("Save").on_click(Command::new(AppVm::save))
```

需要访问运行时服务时，使用带上下文的命令：

```rust
Command::new_with_context(|vm: &mut AppVm, ctx| {
    ctx.log().info("save requested");
    vm.save();
})
```

上下文可访问窗口控制、通知、对话框和日志等服务。

## 文本输入

`Input` 和 `Textarea` 使用 `TextController` 保存文本、选择区间和变更通知。处理表单和编辑器时优先复用控制器，而不是在 widget 外部维护一份孤立字符串。

## 表单

`Form`、`FormField<T>` 和 `TextFormField` 位于 ViewModel 层，用于聚合值、校验、提交和重置。它们不绑定具体视觉样式，适合复用在不同界面布局中。
