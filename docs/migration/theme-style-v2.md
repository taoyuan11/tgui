# Theme and Style API v2 迁移

Theme and Style API v2 引入了生产级主题/样式系统，并移除了旧的 mode-only style 模型。这是一次破坏性变更。

## API 替换

```text
ButtonStyle::default_for(mode)
-> ButtonStyle::default_for_theme(theme, variant)

TextWidgetStyle::default_for(mode)
-> TextWidgetStyle::default_for_theme(theme)

ContainerStyle::default_for(mode)
-> ContainerStyle::default_for_theme(theme)

Stateful<T>
-> StateValue<T>
```

## 主题驱动默认值

默认 widget 样式现在从 `Theme` tokens 读取。运行时解析组件树时会根据主题中的颜色、排版、圆角、间距、边框、焦点环和 elevation 生成默认外观。

这意味着修改 `Theme.colors.primary`、`Theme.typography.body` 或 `Theme.radius` 等 token 后，核心控件会自动获得新的默认样式，而不需要为每个 widget 写局部 style closure。

## StyleSheet 与局部样式

应用级样式优先使用 `Theme.components` 和 `StyleSheet`。widget 局部样式优先使用 mutator 形式，并只 patch 需要改变的字段：

```rust
Button::new("Save").style(|style, ctx| {
    style.radius = ctx.theme.radius.sm.into();
});
```

只有当组件确实需要完整替换 style object 时，才使用 `style_full(...)`。

## 不提供兼容层

旧 mode-only closures、`Stateful<T>` 和 `*Style::default_for(ResolvedThemeMode)` API 已移除，没有 deprecated shim 或兼容 feature。升级时应直接迁移到主题 token 和 `StateValue<T>`。
