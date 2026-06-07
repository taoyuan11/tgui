# 主题与样式

主题系统由 `Theme`、`ThemeMode`、`ThemeSet`、组件主题和设计 token 组成。默认组件样式会从主题 token 中解析，因此改变主题可以影响大部分基础控件外观。

## 核心类型

- `Theme`：颜色、排版、圆角、间距、边框、焦点环、阴影等 token。
- `ThemeMode`：应用声明的 light、dark 或 system 模式。
- `ResolvedThemeMode`：运行时解析后的实际模式。
- `ThemeSet`：管理 light/dark/system 组合。
- `ThemeStore`：运行时主题状态存储。
- `StateValue<T>`：支持不同组件状态下的样式值。

## 主题驱动默认样式

默认 widget 样式从 `Theme` 读取 token。修改颜色、排版、圆角、间距、边框、焦点环或 elevation 后，核心控件会在解析组件树时拿到新的默认外观。

## 局部样式

应用级样式优先使用 `Theme.components` 和 `StyleSheet`。单个 widget 的小范围定制使用 mutator 形式：

```rust
Button::new("Save").style(|style, ctx| {
    style.radius = ctx.theme.radius.sm.into();
});
```

只有在需要完全替换 style object 时，才使用 `style_full(...)`。

## 主题切换

应用可以直接设置主题，也可以通过 ViewModel 状态绑定主题模式：

```rust
Application::new()
    .theme(Theme::dark())
    .bind_theme_mode(AppVm::theme_mode)
```

主题变化可与动画系统配合，为颜色、尺寸、间距等支持插值的属性添加过渡。
