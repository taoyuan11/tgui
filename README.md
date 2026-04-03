# tgui

`tgui` is a modern, GPU-accelerated Rust GUI framework built on top of `wgpu`, `winit`, `cosmic-text`, and `taffy`.

It is designed around a small MVVM-style API:

- `Application` for window and runtime setup
- `Observable<T>` / `Binding<T>` for reactive state
- `Command` / `ValueCommand` for ViewModel actions
- a compact widget tree made of `Text`, `Button`, `Input`, and layout containers

## Status

This project is under active development.

- The runtime, MVVM flow, rendering, layout, and core widgets are usable today.
- The API is still evolving, especially around styling, rendering quality, and higher-level widgets.

## Current Features

- `Application` builder with:
  - window title and size
  - custom fonts and default font
  - fixed theme or bound `ThemeMode`
  - bound window title and clear color
  - global keyboard / mouse input bindings
- MVVM primitives:
  - `Observable<T>`
  - `Binding<T>`
  - `Command`
  - `ValueCommand`
  - `ViewModelContext`
- Widgets:
  - `Text`
  - `Button`
  - `Input`
  - `Container`
  - `Row`
  - `Column`
  - `Grid`
  - `Flex`
  - `Stack`
- Layout powered by `taffy`
- GPU rendering powered by `wgpu`
- Text shaping and rasterization powered by `cosmic-text`
- Public visual styling APIs:
  - `background(...)`
  - `border(...)`
  - `border_width(...)`
  - `border_color(...)`
  - `border_radius(...)`
  - `opacity(...)`
  - `offset(...)`
- Public interaction APIs:
  - `on_click(...)`
  - `on_double_click(...)`
  - `on_mouse_enter(...)`
  - `on_mouse_leave(...)`
  - `on_mouse_move(...)`
  - `on_focus(...)` / `on_blur(...)` on `Button` and `Input`
- Advanced animation APIs:
  - declarative `Binding::animated(...)` with `Transition` or `AnimationSpec<T>`
  - command-style `AnimatedValue<T>` + `ViewModelContext::timeline()`
  - playback controls: delay, repeat, direction, speed, pause/resume/restart/seek/reverse
  - built-in interpolation for `Color`, `f32`, `Point`, and `Insets`
  - layout animation for width, height, gap, padding, margin, and grow
- Runtime theme transitions when switching `ThemeMode`

## Installation

```toml
[dependencies]
tgui = "0.0.4"
```

or use the repository directly:

```toml
[dependencies]
tgui = { git = "https://github.com/nandebishitaoyuan/tgui.git" }
```

## Quick Start

### Basic Window

```rust
use tgui::Application;

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .title("tgui")
        .window_size(960, 640)
        .run()
}
```

### MVVM + Widget Tree

```rust
use tgui::{
    Application, Binding, Button, Column, Command, Insets, Text, ViewModelContext,
};

struct CounterVm {
    count: tgui::Observable<u32>,
}

impl CounterVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            count: ctx.observable(0),
        }
    }

    fn title(&self) -> Binding<String> {
        self.count
            .binding()
            .map(|count| format!("count: {count}"))
    }

    fn increment(&mut self) {
        self.count.update(|count| *count += 1);
    }

    fn view(&self) -> tgui::Element<Self> {
        Column::new()
            .padding(Insets::all(24.0))
            .gap(12.0)
            .child(Text::new(
                self.count
                    .binding()
                    .map(|count| format!("Clicks: {count}")),
            ))
            .child(
                Button::new(Text::new("Increment"))
                    .on_click(Command::new(Self::increment)),
            )
            .into()
    }
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .with_view_model(CounterVm::new)
        .bind_title(CounterVm::title)
        .root_view(CounterVm::view)
        .run()
}
```

## Declarative Animation

Bindings can opt into advanced transitions directly:

```rust
use std::time::Duration;
use tgui::{Color, Insets, Point, Transition};

let color = expanded
    .binding()
    .map(|value| {
        if value {
            Color::hexa(0x2563EBFF)
        } else {
            Color::hexa(0xF97316FF)
        }
    })
    .animated(Transition::ease_out(Duration::from_millis(240)));

let offset = expanded
    .binding()
    .map(|value| {
        if value {
            Point { x: 0.0, y: 0.0 }
        } else {
            Point { x: 0.0, y: 24.0 }
        }
    })
    .animated(Transition::ease_in_out(Duration::from_millis(260)));

let padding = expanded
    .binding()
    .map(|value| {
        if value {
            Insets::symmetric(28.0, 18.0)
        } else {
            Insets::symmetric(16.0, 12.0)
        }
    })
    .animated(
        Transition::ease_in_out(Duration::from_millis(280))
            .delay(Duration::from_millis(20)),
    );
```

Animated bindings work with:

- widget background color
- widget border color / width / radius
- text color
- opacity
- offset
- width / height
- gap / padding / margin / grow
- bound window clear color

`Binding::animated(...)` also accepts `AnimationSpec<T>` when you want to reuse the same playback profile shape as controller-driven timelines.

## Timeline Animation

For command-style animation, create `AnimatedValue<T>` instances and drive them with a timeline controller:

```rust
use std::time::Duration;
use tgui::{
    AnimationCurve, AnimationSpec, Keyframes, Playback, PlaybackDirection, Point,
    ViewModelContext,
};

let offset = ctx.animated_value(Point { x: 0.0, y: 0.0 });
let timeline = ctx
    .timeline()
    .playback(
        Playback::default()
            .repeat(2)
            .direction(PlaybackDirection::Alternate),
    )
    .track(
        offset.clone(),
        AnimationSpec::from(
            Keyframes::timed(Duration::from_millis(800))
                .curve(AnimationCurve::EaseInOutCubic)
                .at(Duration::ZERO, Point { x: 0.0, y: 0.0 })
                .at(Duration::from_millis(400), Point { x: 0.0, y: 24.0 })
                .at(Duration::from_millis(800), Point { x: 0.0, y: -12.0 }),
        ),
    )
    .build();

timeline.play();
timeline.pause();
timeline.resume();
timeline.seek_percent(0.5);
timeline.reverse();
```

## Styling and Interaction

All core widgets and layout containers support the same style-oriented builder pattern:

```rust
use tgui::{Color, Command, Point, Stack, ValueCommand};

let card = Stack::new()
    .size(200.0, 200.0)
    .background(Color::rgb(255, 255, 255))
    .border(2.0, Color::rgb(0, 0, 0))
    .border_radius(24.0)
    .opacity(0.96)
    .offset(Point { x: 0.0, y: 8.0 })
    .on_click(Command::new(|_| {}))
    .on_mouse_move(ValueCommand::new(|_, point| {
        let _ = (point.x, point.y);
    }));
```

Mouse interaction APIs are available on layout widgets as well, not only on buttons.

## Themes and Fonts

Set a fixed theme:

```rust
use tgui::{Application, Theme};

Application::new().theme(Theme::dark());
```

Or bind the theme mode from the ViewModel:

```rust
use tgui::{Application, ThemeMode};

Application::new()
    .with_view_model(App::new)
    .bind_theme_mode(App::theme_mode)
    .root_view(App::view);
```

Theme transitions are animated by the runtime when the bound `ThemeMode` changes.

Register custom fonts and optionally choose a default font:

```rust
Application::new()
    .font("ui", include_bytes!("./assets/YourFont.ttf"))
    .default_font("ui");
```

Use a specific font on a `Text` widget:

```rust
use tgui::Text;

let title = Text::new("Hello tgui")
    .font("ui")
    .font_size(24.0);
```

## Examples

Available examples in this repository:

- `basic_window`
- `mvvm_counter`
- `animation_showcase`
- `timeline_controller`
- `theme`
- `input`
- `layout`
- `layout_theme_showcase`
- `widgets_showcase`

Run them with:

```bash
cargo run --example basic_window
cargo run --example mvvm_counter
cargo run --example animation_showcase
cargo run --example timeline_controller
cargo run --example theme
cargo run --example input
```

## Development

```bash
cargo fmt
cargo check
cargo check --examples
cargo test
```

## Current Limitations

- The widget set is still intentionally small.
- Enter / exit lifecycle animation is not implemented yet.
- Rendering and styling quality are improving quickly, so visual details may still change between releases.
- Public APIs are not frozen yet.

## License

MIT
