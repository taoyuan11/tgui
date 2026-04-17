# tgui

[![zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/taoyuan11/tgui)

`tgui` is a modern, GPU-accelerated Rust GUI framework built on top of `wgpu`, `winit-core`, platform backends, `cosmic-text`, and `taffy`.

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
  - desktop multi-window reconciliation driven by shared view-model state
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
  - `Image`
  - `Container`
  - `Row`
  - `Column`
  - `Grid`
  - `Flex`
  - `Stack`
- Layout powered by `taffy`
- GPU rendering powered by `wgpu`
- Text shaping and rasterization powered by `cosmic-text`
- Media loading:
  - `Image` supports local files plus `http/https` sources on all current targets
- Public visual styling APIs:
  - `background(...)`
  - `border(...)`
  - `border_width(...)`
  - `border_color(...)`
  - `border_radius(...)`
  - `opacity(...)`
  - `offset(...)`
  - `overflow(...)`
  - `overflow_x(...)`
  - `overflow_y(...)`
- Public interaction APIs:
  - `on_click(...)`
  - `on_double_click(...)`
  - `on_mouse_enter(...)`
  - `on_mouse_leave(...)`
  - `on_mouse_move(...)`
  - `on_focus(...)` / `on_blur(...)` on `Button` and `Input`
- Built-in desktop file dialogs:
  - synchronous modal file/folder/save dialogs via `Command::new_with_context(...)`
  - asynchronous file/folder/save dialogs with result callbacks on the shared view model
- Built-in desktop message dialogs:
  - synchronous and asynchronous alert/confirm style dialogs
  - owner-bound to the current runtime window automatically
- Advanced animation APIs:
  - declarative `Binding::animated(...)` with `Transition` or `AnimationSpec<T>`
  - command-style `AnimatedValue<T>` + `ViewModelContext::timeline()`
  - playback controls: delay, repeat, direction, speed, pause/resume/restart/seek/reverse
  - built-in interpolation for `Color`, `f32`, `Point`, and `Insets`
  - layout animation for width, height, gap, padding, margin, and grow
- Runtime theme transitions when switching `ThemeMode`
- Container overflow clipping with optional bi-directional mouse-wheel scrolling
- Visual scrollbars for scrollable containers with customizable styling
- Android support behind the `android` feature:
  - NativeActivity runtime
  - touch input
  - `ThemeMode::System` integration
  - Android system font discovery
  - foreground / background surface recovery
- OHOS support behind the `ohos` feature:
  - ArkUI `NativeXComponent` backend via `tgui-winit-ohos`
  - `cargo ohos-app` packaging flow
  - touch, mouse, keyboard, and surface lifecycle integration
  - OHOS system font discovery

## Installation

For desktop targets:

```toml
[dependencies]
tgui = "0.1.2"
```

or use the repository directly:

```toml
[dependencies]
tgui = { git = "https://github.com/nandebishitaoyuan/tgui.git" }
```

For Android targets, enable the `android` feature:

```toml
[dependencies]
tgui = { version = "0.1.2", features = ["android"] }
```

If you create an Android NativeActivity app directly, add the matching modular backend crates:

```toml
[target.'cfg(target_os = "android")'.dependencies]
winit-core = "0.31.0-beta.2"
winit-android = { version = "0.31.0-beta.2", features = ["native-activity"] }
```

For OHOS targets, enable the `ohos` feature:

```toml
[dependencies]
tgui = { version = "0.1.2", features = ["ohos"] }
```

If you package an OHOS app with `cargo ohos-app`, the packager can now detect
`tgui-winit-ohos` directly, so a separate `winit-ohos` shim dependency is no longer required.

A minimal dependency setup looks like this:

```toml
[dependencies]
winit-core = "0.31.0-beta.2"
tgui-winit-ohos = "0.31.0-beta.2"
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

### Media Widgets

```rust
use tgui::{Application, Column, Command, ContentFit, Image, Insets, ValueCommand};

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .with_view_model(|_| ())
        .root_view(|_| {
            Column::new()
                .padding(Insets::all(24.0))
                .gap(16.0)
                .child(
                    Image::from_url("https://example.com/cover.jpg")
                        .height(220.0)
                        .fill_width()
                        .fit(ContentFit::Cover)
                        .on_loading(Command::new(|_| println!("image loading")))
                        .on_success(Command::new(|_| println!("image ready")))
                        .on_error(ValueCommand::new(|_, error| eprintln!("{error}"))),
                )
                .into()
        })
        .run()
}
```

When you want layout to stay stable before media finishes loading, set an
explicit size or `aspect_ratio(...)`. `Image` exposes
`on_loading(...)`, `on_success(...)`, and `on_error(...)`.

### Desktop Multi-Window

On desktop targets, one shared view model can reconcile one main window plus multiple child windows:

```rust
use tgui::{Application, Text, ViewModelContext, WindowSpec};

struct AppVm;

impl AppVm {
    fn new(_: &ViewModelContext) -> Self {
        Self
    }

    fn windows(&self) -> Vec<WindowSpec<Self>> {
        vec![
            WindowSpec::main("main")
                .title("Main")
                .window_size(960, 640)
                .root_view(|_| Text::new("Main window").into()),
            WindowSpec::child("inspector")
                .title("Inspector")
                .window_size(420, 320)
                .root_view(|_| Text::new("Inspector").into()),
        ]
    }
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .close_children_with_main(true)
        .with_view_model(AppVm::new)
        .windows(AppVm::windows)
        .run()
}
```

### Built-in File Dialogs

Use `Command::new_with_context(...)` when a handler needs runtime services such
as desktop dialogs:

```rust
use tgui::{Application, Button, Column, Command, FileDialogOptions, Text, ViewModelContext};

struct AppVm {
    selected: tgui::Observable<String>,
}

impl AppVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            selected: ctx.observable("Nothing selected".to_string()),
        }
    }

    fn view(&self) -> tgui::Element<Self> {
        Column::new()
            .child(
                Button::new(Text::new("Open file"))
                    .on_click(Command::new_with_context(|vm, ctx| {
                        if let Ok(Some(path)) = ctx.dialogs().open_file(
                            FileDialogOptions::new().add_filter("Text", &["txt", "md"]),
                        ) {
                            vm.selected.set(path.display().to_string());
                        }
                    })),
            )
            .child(Text::new(self.selected.binding()))
            .into()
    }
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .with_view_model(AppVm::new)
        .root_view(AppVm::view)
        .run()
}
```

### Built-in Message Dialogs

Use the same command context to show owner-bound native message dialogs:

```rust
use tgui::{
    Application, Button, Column, Command, MessageDialogButtons, MessageDialogLevel,
    MessageDialogOptions, MessageDialogResult, Text, ViewModelContext,
};

struct AppVm {
    status: tgui::Observable<String>,
}

impl AppVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            status: ctx.observable("Waiting".to_string()),
        }
    }

    fn view(&self) -> tgui::Element<Self> {
        Column::new()
            .child(
                Button::new(Text::new("Confirm"))
                    .on_click(Command::new_with_context(|vm, ctx| {
                        let result = ctx.dialogs().show_message(
                            MessageDialogOptions::new()
                                .title("Confirm action")
                                .description("Proceed with the change?")
                                .level(MessageDialogLevel::Warning)
                                .buttons(MessageDialogButtons::YesNo),
                        );

                        vm.status.set(match result {
                            Ok(MessageDialogResult::Yes) => "Confirmed".to_string(),
                            Ok(other) => format!("Dismissed: {other:?}"),
                            Err(error) => format!("Dialog failed: {error}"),
                        });
                    })),
            )
            .child(Text::new(self.status.binding()))
            .into()
    }
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .with_view_model(AppVm::new)
        .root_view(AppVm::view)
        .run()
}
```

## Core Concepts

Most `tgui` applications follow the same flow:

1. Build an [`Application`](https://docs.rs/tgui/latest/tgui/struct.Application.html) for window, theme, and font setup.
2. Create state inside a `ViewModel` with `ViewModelContext`.
3. Store mutable UI state in `Observable<T>`.
4. Expose render-ready values with `Binding<T>`.
5. Connect events back to the view model with `Command` or `ValueCommand`.
6. Return a widget tree from `root_view(...)`.

Use each primitive for a specific job:

- `Observable<T>`: regular reactive state such as counters, text input, filters, selection, and toggles.
- `Binding<T>`: derived values for text, colors, spacing, size, visibility, and window-level properties.
- `Binding::animated(...)`: declarative transitions for values that should interpolate automatically.
- `AnimatedValue<T>`: imperative animation targets driven by a controller or timeline.
- `Command` / `ValueCommand`: event handlers for clicks, focus changes, pointer movement, and input updates.
- `CommandContext`: runtime services passed into `Command::new_with_context(...)` and `ValueCommand::new_with_context(...)`.

The framework does not require a separate message loop or reducer layer. State changes on `Observable<T>` automatically invalidate the UI and trigger recomposition of the affected bindings during the next frame.

## Recommended App Structure

For medium-sized apps, this shape tends to work well:

```rust
struct AppVm {
    title: tgui::Observable<String>,
    count: tgui::Observable<u32>,
    // other screen state...
}

impl AppVm {
    fn new(ctx: &tgui::ViewModelContext) -> Self { /* ... */ unimplemented!() }
    fn title(&self) -> tgui::Binding<String> { /* ... */ unimplemented!() }
    fn view(&self) -> tgui::Element<Self> { /* ... */ unimplemented!() }
}
```

Practical guidance:

- Keep source-of-truth state in `Observable<T>`.
- Keep formatting logic in `Binding::map(...)`.
- Keep side-effect-free rendering in `view(...)`.
- Keep mutations in small view-model methods, then wrap them with `Command::new(...)`.
- Prefer binding window title, clear color, and theme mode instead of pushing those updates manually.

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

Containers clip overflow by default. Opt into visibility or scrolling per axis:

```rust
use tgui::{Column, Overflow, ScrollbarStyle};

let list = Column::new()
    .height(320.0)
    .overflow_y(Overflow::Scroll)
    .scrollbar_style(
        ScrollbarStyle::default()
            .thickness(10.0)
            .hover_thumb_color(tgui::Color::hexa(0x67E8F9F2))
            .active_thumb_color(tgui::Color::WHITE)
            .min_thumb_length(36.0),
    );
```

Scrollbar styling can also be tuned with convenience builders on any layout container:

- `scrollbar_style(...)`
- `scrollbar_thumb_color(...)`
- `scrollbar_hover_thumb_color(...)`
- `scrollbar_active_thumb_color(...)`
- `scrollbar_track_color(...)`
- `scrollbar_thickness(...)`
- `scrollbar_radius(...)`
- `scrollbar_insets(...)`
- `scrollbar_min_thumb_length(...)`

Scrollbars respond to both mouse-wheel scrolling and direct thumb dragging.

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

On Android, `tgui` also loads system fonts automatically, so a manually registered default font is not required just to make the app start or render text.

## Android

Android support is feature-gated:

```toml
[dependencies]
tgui = { path = "../..", features = ["android"] }

[target.'cfg(target_os = "android")'.dependencies]
winit-core = "0.31.0-beta.2"
winit-android = { version = "0.31.0-beta.2", features = ["native-activity"] }
```

The current Android path is based on `NativeActivity`. `ThemeMode::System`, touch interaction, system font discovery, and background-to-foreground resume are supported in the runtime.

For direct window-system types, prefer importing them from `tgui::platform` instead of depending on the old aggregate `winit` crate.

### Minimal Android Project Setup

A minimal Android example project looks like this:

```toml
[package]
name = "android_basic_window"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
tgui = { path = "../..", features = ["android"] }

[target.'cfg(target_os = "android")'.dependencies]
winit = { version = "0.30", features = ["android-native-activity"] }

[package.metadata.android.sdk]
min_sdk_version = 23
target_sdk_version = 34

[package.metadata.android.application.activity]
config_changes = "orientation|keyboardHidden|screenSize|screenLayout|uiMode"

[lib]
crate-type = ["cdylib"]
```

The Rust entry point should export `android_main`:

```rust
#[cfg(target_os = "android")]
use tgui::{Application, TguiError};
#[cfg(target_os = "android")]
use tgui::platform::android::activity::AndroidApp;

#[cfg(target_os = "android")]
fn run_android_entry(app: AndroidApp) -> Result<(), TguiError> {
    Application::new()
        .title("tgui android")
        .run_android(app)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: AndroidApp) {
    if let Err(error) = run_android_entry(app) {
        panic!("failed to run android app: {error}");
    }
}
```

### Build And Run

1. Set Android toolchain environment variables:

```bash
ANDROID_HOME=/path/to/Android/Sdk
ANDROID_NDK_HOME=/path/to/Android/Sdk/ndk/<version>
```

2. Build the Android APK:

```bash
cargo apk build --manifest-path examples/android_basic_window/Cargo.toml --target x86_64-linux-android
```

For a physical ARM64 device, use:

```bash
cargo apk build --manifest-path examples/android_basic_window/Cargo.toml --target aarch64-linux-android
```

3. Install to a connected device or emulator:

```bash
adb install -r examples/android_basic_window/target/debug/apk/android_basic_window.apk
```

4. Launch the example manually if needed:

```bash
adb shell am start -n rust.android_basic_window/android.app.NativeActivity
```

The example manifest package is currently `rust.android_basic_window`, so the launch command above matches the generated APK.

## OHOS

OHOS support is feature-gated:

```toml
[dependencies]
tgui = { path = "../..", features = ["ohos"] }

[lib]
crate-type = ["staticlib", "rlib"]
```

The current OHOS path targets ArkUI `NativeXComponent` through `tgui-winit-ohos`. For direct
backend types and macros, import them from `tgui::platform::ohos`.

When packaging with `cargo ohos-app`, adding `tgui` with the `ohos` feature is enough for the
packager to discover the backend through `tgui-winit-ohos`.

### Minimal OHOS Runtime Export

```rust
#[cfg(target_env = "ohos")]
use tgui::platform::ohos::export_ohos_winit_app;
#[cfg(target_env = "ohos")]
use tgui::Application;
#[cfg(target_env = "ohos")]
use winit_core::application::ApplicationHandler;

#[cfg(target_env = "ohos")]
fn create_ohos_app() -> impl ApplicationHandler + Send {
    Application::new()
        .title("tgui ohos")
        .into_ohos_handler()
}

#[cfg(target_env = "ohos")]
export_ohos_winit_app!(create_ohos_app);
```

### Build And Package

1. Install the cargo subcommand:

```bash
cargo install cargo-ohos-app
```

2. Generate the OHOS shell project if needed:

```bash
cargo ohos-app init --manifest-path examples/ohos_basic_window/Cargo.toml
```

3. Package the example as a `.hap`:

```bash
cargo ohos-app package --manifest-path examples/ohos_basic_window/Cargo.toml
```

The bundled example defaults to `x86_64-unknown-linux-ohos` in
`examples/ohos_basic_window/ohos-app.toml`, which is convenient for simulator runs. For a device
build, pass `--abi arm64-v8a` or override `target` in that file.

## Examples

Available examples in this repository:

- `basic_window`
- `mvvm_counter`
- `animation_showcase`
- `timeline_controller`
- `theme`
- `input`
- `layout`
- `scroll`
- `layout_theme_showcase`
- `widgets_showcase`
- `media_showcase`
- `multi_window`
- `android_basic_window`
- `ohos_basic_window`

All runnable examples now live in their own Cargo projects under `examples/<name>/`.
`examples/static` is a shared assets folder, not a standalone example project.

Run desktop examples with `--manifest-path`:

```bash
cargo run --manifest-path examples/basic_window/Cargo.toml
cargo run --manifest-path examples/mvvm_counter/Cargo.toml
cargo run --manifest-path examples/animation_showcase/Cargo.toml
cargo run --manifest-path examples/timeline_controller/Cargo.toml
cargo run --manifest-path examples/theme/Cargo.toml
cargo run --manifest-path examples/input/Cargo.toml
cargo run --manifest-path examples/layout/Cargo.toml
cargo run --manifest-path examples/scroll/Cargo.toml
cargo run --manifest-path examples/layout_theme_showcase/Cargo.toml
cargo run --manifest-path examples/widgets_showcase/Cargo.toml
cargo run --manifest-path examples/media_showcase/Cargo.toml
cargo run --manifest-path examples/multi_window/Cargo.toml
```

Build the Android example with:

```bash
cargo apk build --manifest-path examples/android_basic_window/Cargo.toml --target x86_64-linux-android
```

The Android example already enables `tgui`'s `android` feature in its own `Cargo.toml`, so you do not need to pass `--features android` on the command line.

Package the OHOS example with:

```bash
cargo ohos-app package --manifest-path examples/ohos_basic_window/Cargo.toml
```

The OHOS example already enables `tgui`'s `ohos` feature, and `cargo ohos-app` can detect
`tgui-winit-ohos` transitively to generate the `XComponent` shell automatically.

## Development

```bash
cargo fmt
cargo check
cargo test
```

## Current Limitations

- The widget set is still intentionally small.
- Enter / exit lifecycle animation is not implemented yet.
- Rendering and styling quality are improving quickly, so visual details may still change between releases.
- Public APIs are not frozen yet.

## License

MIT
