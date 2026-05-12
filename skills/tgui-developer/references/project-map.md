# tgui Project Map

## Identity

`tgui` is a Rust 2021 crate for GPU-accelerated GUI applications. It combines `wgpu` rendering, `winit-core` platform backends, `taffy` layout, `cosmic-text`, a small MVVM layer, themes, animation, text input, media loading, native dialogs, custom window chrome/native window control, canvas drawing, and optional FFmpeg video.

Crate metadata: package `tgui`, current version `0.1.8`, edition `2021`, license MIT. Major dependencies include `wgpu`, `winit-core` plus platform backends, `taffy`, `cosmic-text`, `image`, `resvg`, `reqwest`, `lyon`, and optional `ffmpeg-next`.

## Key Files

- `Cargo.toml`: crate metadata, features, target-specific dependencies, publish excludes.
- `src/lib.rs`: public API exports and `prelude`.
- `src/application/mod.rs`: `Application`, `WindowSpec`, multi-window declarations, window decoration configuration, window bindings, platform run entry points.
- `src/foundation/binding.rs`: `ViewModelContext`, `State`, `Signal`, `TextController`, dependency tracking, invalidation.
- `src/foundation/view_model.rs`: `ViewModel`, `Command`, `ValueCommand`, `CommandContext`.
- `src/foundation/window_control.rs`: `WindowControl`, `WindowResizeDirection`, and queued native window requests for command handlers.
- `src/runtime/`: event loop integration, window lifecycle, input, focus, scrolling, text editing, commands, window control request draining, dialog callbacks, theme binding, animation refresh, media state, render scheduling.
- `src/ui/layout.rs`: layout value types such as `Length`, `Track`, `Insets`, `Align`, `Justify`, `Axis`, `Overflow`, `Value`.
- `src/ui/widget/core/`: element tree resolution, Taffy layout, scene primitive collection, hit regions, scrolling, input/editing, selection. High-risk area.
- `src/ui/widget/*.rs`: public widget builders such as button, text, input, textarea, image, checkbox, radio, select, switch, canvas, background, video.
- `src/ui/theme/`: theme tokens, component themes, state resolution, light/dark/system mode.
- `src/rendering/renderer.rs`: `wgpu` renderer and pipelines for rects, brushes, meshes, text, textures, transparent window surfaces, backdrop blur.
- `src/rendering/shader/*.wgsl`: shader code.
- `src/media/mod.rs`: raster image/SVG/network/memory loading, texture and shadow caches.
- `src/dialog.rs`: native dialogs through `rfd` on desktop; unsupported stubs on Android/OHOS.
- `src/notification.rs`: notifications, permissions, platform dispatch, and interactive action callbacks.
- `src/platform.rs`: platform abstraction and selected winit backend.
- `src/video/`: `video` feature API and FFmpeg backend.
- `examples/`: independent Cargo examples.

## Features

- `default = []`
- `android`: Android entry and `winit-android`.
- `ohos`: HarmonyOS/OpenHarmony entry and `tgui-winit-ohos`.
- `video`: enables `ffmpeg-next`.
- `video-static`: enables `video` plus `ffmpeg-next/static`.

Desktop targets use windowing, clipboard, dialog, raw-window-handle, logging, and audio dependencies. Android uses `jni` and `winit-android`. OHOS uses `hilog-sys` and `tgui-winit-ohos`. Windows video builds link extra system libraries in `build.rs`.

## Public API Groups

- `application`: `Application`, `WindowSpec`, `WindowRole`, `WindowClosePolicy`.
- `mvvm`: `ViewModel`, `ViewModelContext`, `State`, `Signal`, `TextController`, `TextChange`, `TextChangeSet`, `TextSnapshot`, `Command`, `ValueCommand`, `CommandContext`, `WindowControl`, `WindowResizeDirection`.
- `layout`: `Flex`, `Grid`, `Stack`, `Length`, `Track`, `Insets`, alignment, overflow, units, `Value`.
- `widgets`: `Button`, `Text`, `Input`, `Textarea`, `Image`, `Checkbox`, `Radio`, `Select`, `Switch`, `Element`, `WidgetTree`, common styling.
- `canvas`: `Canvas`, `PathBuilder`, canvas paths, gradients, shadows, boolean ops, pointer events.
- `theme`: `Theme`, `ThemeMode`, `ThemeSet`, design tokens, component styles.
- `media`: `MediaSource`, `MediaBytes`, `ContentFit`.
- `dialog`: file and message dialog types.
- `notification`: `NotificationOptions`, `NotificationAction`, `NotificationActionEvent`, `NotificationPermission`, `Notifications`.
- `video`: exported only with the `video` feature.
- `prelude`: convenient import set for examples and small apps.

## Application and Window Control

Typical startup is MVVM-only:

```rust
Application::new()
    .title("demo")
    .window_size(dp(960.0), dp(640.0))
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

`Application` also supports `bind_title`, `bind_clear_color`, `bind_theme_mode`, and `on_input` for window-level bindings and shortcuts.

Use `Application::decorations(false)` or `WindowSpec::decorations(false)` to disable native system decorations. For transparent custom chrome, also set `clear_color(Color::TRANSPARENT)`.

`Command::new_with_context` and `ValueCommand::new_with_context` can access `CommandContext::window()`. `WindowControl` can request native window drag, drag-resize from a `WindowResizeDirection`, minimize, maximize, restore, toggle maximize, close, and query `is_maximized()`. Requests are queued from command handlers and drained by the runtime so view-model callbacks do not directly mutate platform windows.

## State and Text Flow

- Create reactive values with `ViewModelContext::state`.
- Read state through `State::signal()` and derive UI-facing values with `Signal::map`.
- Use `Signal::animated(Transition)` for declarative property transitions on supported types.
- Use `TextController` for retained `Input` and `Textarea` content. Text-editing changes often cross `src/ui/widget/common.rs`, `src/ui/widget/core/`, and `src/runtime/input.rs`.
- `TextChangeSet` and `TextSnapshot` are the main payload and inspection types for text editing callbacks and tests.

## Runtime Flow

1. A `ViewModel` builds an `Element<VM>` tree.
2. `WidgetTree` resolves the tree and computes layout with Taffy.
3. Widgets emit scene primitives, hit regions, scroll areas, IME/caret state, and command targets.
4. `src/runtime/` processes platform events, input, hover/focus/pressed state, command dispatch, window control requests, cache invalidation, media/dialog callbacks, and redraw scheduling.
5. `Renderer` submits primitives to `wgpu` pipelines.

Transparent windows are driven by clear color alpha. The renderer picks non-opaque composite alpha modes for transparent surfaces; on Windows transparent windows prefer DX12 and a DXGI visual swapchain path.

## Widget Change Checklist

- Add or update the builder API in the relevant `src/ui/widget/*.rs` file.
- Store layout/visual/interaction state using existing structs where possible.
- Wire behavior into `WidgetKind`/core tree handling only where needed.
- Include hit-testing, focus, pressed/hover state, scroll behavior, text selection, IME behavior, and change-set emission when the widget participates in those systems.
- Emit scene primitives compatible with `src/rendering/renderer.rs`.
- Expose public types through `src/lib.rs` if the API is meant for users.
- Add focused tests near existing widget/core tests and update examples for user-facing APIs.

## Window Control Checklist

- Keep application-level and per-window `decorations` settings in sync between `Application`, `ApplicationConfig`, `WindowSpec`, window creation, and runtime reconfiguration.
- Route user-facing native window actions through `CommandContext::window()` and `WindowControl`; avoid direct platform window mutation from view-model code.
- Update multi-window behavior when adding close/minimize/maximize semantics, especially main-window close policy and child-window cleanup.
- Check renderer alpha mode/backend behavior when custom chrome depends on transparent windows.
- Update `examples/frameless_window` or add a focused example/test when changing the custom chrome API.

## Validation Targets

- Layout, primitive, input, selection, scroll, and widget state: `src/ui/widget/core/tests.rs`.
- Runtime focus, text input editing, scrollbars, command dispatch, canvas/video hit behavior: `src/runtime/tests.rs`.
- Window decoration config and command window control: `src/application/mod.rs` and `src/foundation/window_control.rs` tests.
- Media, SVG, rasterization, external resources, caches: `src/media/mod.rs` tests.
- Animation and timelines: `src/animation.rs` tests.
- Theme state and tokens: `src/ui/theme/mod.rs` tests.
- Font behavior: `src/text/font.rs` tests.
- Canvas-specific behavior: `src/ui/widget/canvas.rs` tests.
- Video controller/backend: `src/video/**` tests with the appropriate feature and local FFmpeg environment.

## Actual Examples To Check

Use `rg --files examples -g Cargo.toml` before editing docs because README prose can lag behind the directory. Current examples include:

- `basic_window`
- `mvvm_counter`
- `animation_showcase`
- `timeline_controller`
- `multi_window`
- `dialogs`
- `canvas`
- `background_effects`
- `frameless_window`
- `demo`
- `text_area`
- `multiple_vm_examples`
- `android_basic_window`
- `ohos_basic_window`

Run examples with:

```powershell
cargo run --manifest-path examples/basic_window/Cargo.toml
cargo run --manifest-path examples/mvvm_counter/Cargo.toml
cargo run --manifest-path examples/canvas/Cargo.toml
cargo run --manifest-path examples/frameless_window/Cargo.toml
```

## Maintenance Notes

- Do not treat `src/runtime/` or `src/ui/widget/core/` as small utility modules; changes can affect input, layout, cache invalidation, rendering, commands, and platform event behavior.
- Public API changes should be checked against `src/lib.rs` re-exports, README/docs, examples, and tests.
- `Cargo.toml` excludes `examples/*`, `*.png`, `.github/*`, `AGENTS.md`, and `skills/*` from publication; verify resource packaging for release-facing changes.
- Add new platform behavior behind the existing `cfg` structure and platform abstraction.
- Text changes must respect UTF-8 boundaries, IME composition, selection ranges, caret visibility, and horizontal scrolling.
- Async media/dialog completions must trigger invalidation through the runtime.
- Do not delete, rename, or overwrite the untracked `Video.md` unless explicitly asked.
