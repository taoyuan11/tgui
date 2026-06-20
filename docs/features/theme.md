# 主题与样式

主题系统由 `Theme`、`ThemeMode`、`ThemeSet`、设计 token、组件主题、`StyleSheet` 和局部 `style(...)` 组成。推荐顺序是：先用主题 token 建立全局风格，再用 `StyleSheet` 做应用级规则，最后用单个组件的 `style(...)` 处理局部差异。

## 核心类型

| 类型 | 作用 |
| --- | --- |
| `Theme` | 当前解析后的颜色、排版、圆角、间距、边框、焦点环、阴影、动效和组件主题。 |
| `ThemeBuilder` | 创建自定义 light / dark 主题。 |
| `ThemeMode` | 应用声明的 `Light`、`Dark` 或 `System`。 |
| `ThemeSet` | 同时保存 light 和 dark 主题，按 `ThemeMode` 解析。 |
| `ThemeStore` | 运行时主题状态存储。通常应用侧不直接操作。 |
| `StateValue<T>` | 为 normal / hovered / pressed / disabled / invalid 等状态提供不同样式值。 |
| `StyleContext` | style closure 中的上下文，包含当前主题等信息。 |
| `StyleSheet` | 应用级样式规则，可按 class、style id、组件状态和按钮变体匹配。 |

## 创建主题

最小用法是设置主题模式：

```rust
Application::new()
    .theme_mode(ThemeMode::System)
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

自定义 light/dark 主题时，使用 `ThemeBuilder` 和 `ThemeSet`：

```rust
let light = ThemeBuilder::light("acme-light")
    .primary(Color::hexa(0x2563EBFF))
    .build();

let dark = ThemeBuilder::dark("acme-dark")
    .primary(Color::hexa(0x38BDF8FF))
    .build();

Application::new()
    .theme_set(ThemeSet::new(light, dark))
    .theme_mode(ThemeMode::System)
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

`ThemeBuilder` 可继续配置：

| API | 说明 |
| --- | --- |
| `primary(color)` | 设置主题主色。 |
| `colors(ColorScheme)` | 替换完整颜色方案。 |
| `typography(TypeScale)` | 替换字体尺寸和字重 token。 |
| `spacing(SpaceScale)` | 替换间距 token。 |
| `radius(RadiusScale)` | 替换圆角 token。 |
| `border(BorderScale)` | 替换边框宽度 token。 |
| `focus_ring(FocusRingStyle)` | 设置焦点环样式。 |
| `elevation(ElevationScale)` | 设置阴影/elevation token。 |
| `motion(MotionScale)` | 设置默认动效 token。 |
| `density(Density)` | 调整控件密度。 |
| `components(ComponentThemes)` | 覆盖组件默认主题。 |

## 绑定主题模式

主题模式可以由 ViewModel 状态控制：

```rust
struct AppVm {
    theme: State<ThemeMode>,
}

impl AppVm {
    fn theme_mode(&self) -> Signal<ThemeMode> {
        self.theme.signal()
    }

    fn set_theme(&mut self, mode: ThemeMode) {
        self.theme.set(mode);
    }
}

Application::new()
    .with_view_model(AppVm::new)
    .bind_theme_mode(AppVm::theme_mode)
    .root_view(AppVm::view)
    .run()
```

```rust
RadioGroup::new(
    vec![
        RadioOption::new(ThemeMode::System, "跟随系统".to_string()),
        RadioOption::new(ThemeMode::Light, "明亮".to_string()),
        RadioOption::new(ThemeMode::Dark, "暗色".to_string()),
    ],
    self.theme.signal(),
)
.on_change(ValueCommand::new(|vm: &mut AppVm, (mode, _label)| {
    vm.set_theme(mode);
}))
```

## 局部样式

单个组件的小范围定制优先使用 `style(...)`。它先取主题默认样式，再让闭包修改需要变化的字段。

```rust
Button::new("保存")
    .primary()
    .style(|style, ctx| {
        style.radius = ctx.theme.radius.sm.into();
        style.padding_x = ctx.theme.spacing.md;
    })
```

完全替换 style object 时使用 `style_full(...)`：

```rust
Text::new("状态正常")
    .style_full(|ctx| {
        let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
        style.typography.size = sp(14.0);
        style.color = Color::hexa(0x16A34AFF).into();
        style
    })
```

经验规则：

- 只改几个字段时用 `style(...)`。
- 需要从零构造样式对象时用 `style_full(...)`。
- 样式里需要当前主题 token 时，从 `ctx.theme` 读取。
- 不要为了全局风格在每个组件上重复写同一段 closure，使用 `ThemeSet` 或 `StyleSheet`。

## StateValue

交互控件的颜色、边框等字段通常是 `StateValue<Value<Color>>`，可以给不同状态提供不同值。

```rust
Button::new("运行")
    .style(|style, _ctx| {
        style.background = StateValue::interactive(
            Color::hexa(0x2563EBFF).into(),
            Color::hexa(0x1D4ED8FF).into(),
            Color::hexa(0x1E40AFFF).into(),
            Color::hexa(0x94A3B8FF).into(),
        );
        style.foreground = StateValue::new(Color::WHITE.into());
    })
```

状态优先级由运行时解析，常见状态包括 hovered、pressed、disabled、focused、focus_visible、selected、checked、open 和 invalid。

## StyleSheet

`StyleSheet` 适合应用级复用规则。组件可以通过 `.class(...)` 或 `.style_id(...)` 参与匹配。

```rust
let sheet = StyleSheet::new()
    .button(
        ButtonSelector::primary().class("toolbar"),
        |style, ctx| {
            style.radius = ctx.theme.radius.sm.into();
            style.min_height = dp(30.0);
        },
    )
    .text_class("muted", |style, ctx| {
        style.color = ctx.theme.colors.on_surface_muted.into();
        style.typography.size = sp(13.0);
    })
    .container_id("settings-panel", |style, ctx| {
        style.surface.background = Some(ctx.theme.colors.surface.into());
        style.surface.border_radius = Some(ctx.theme.radius.md.into());
    });

Application::new()
    .style_sheet(sheet)
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

```rust
Flex::vertical()
    .style_id("settings-panel")
    .child(el![
        Text::new("高级设置").class("muted"),
        Button::new("同步").primary().class("toolbar"),
    ])
```

`StyleSelector` 可匹配 class、style id 和 widget state；`ButtonSelector` 额外支持按钮变体和尺寸。规则按组件类型注册，例如 `text_class` 只影响 `Text`，`input_class` 只影响 `Input`。

## 组件主题与局部样式的边界

| 需求 | 推荐方式 |
| --- | --- |
| 改整个应用的主色、背景、文字、圆角、间距 | `ThemeBuilder` / `ThemeSet` |
| 改某一类组件默认外观 | `ComponentThemes` 或 `StyleSheet` |
| 按业务区域复用一组样式 | `StyleSheet` + `.class(...)` |
| 精确修改一个组件 | 组件 `.style(...)` |
| 完全替换一个组件样式对象 | `.style_full(...)` |
| 主题切换时让颜色平滑过渡 | `Signal::animated(Transition)` |

## 动画和 reduced motion

主题值可以和动画系统配合。例如主题模式变化时，为颜色或尺寸信号添加过渡：

```rust
let transition = Transition::ease_in_out(std::time::Duration::from_millis(180));

Stack::new()
    .background(self.panel_color().animated(transition))
```

应用可以设置 reduced motion 默认值，也可以绑定状态：

```rust
Application::new()
    .reduced_motion(false)
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

## 迁移提示

旧的 mode-only style API 已被主题 token 和 `StateValue<T>` 替代。升级旧代码时：

- `ButtonStyle::default_for(mode)` 改为 `ButtonStyle::default_for_theme(theme, variant)`。
- `TextWidgetStyle::default_for(mode)` 改为 `TextWidgetStyle::default_for_theme(theme)`。
- `ContainerStyle::default_for(mode)` 改为 `ContainerStyle::default_for_theme(theme)`。
- `Stateful<T>` 改为 `StateValue<T>`。

详细迁移说明见[Theme and Style API v2](/migration/theme-style-v2)。
