# tgui Project Map

## Identity

`tgui` is a Rust 2021 workspace for GPU-accelerated GUI applications. It combines `wgpu` rendering, `winit-core` platform backends, `taffy` layout, `cosmic-text`, a small MVVM layer, themes, animation, text input, media loading, native dialogs, custom window chrome/native window control, canvas drawing, and optional FFmpeg-backed audio/video.

Workspace metadata: public package `tgui`, current version `0.2.0`, edition `2021`, rust-version `1.85`, license `MIT OR Apache-2.0`. Major dependencies include `wgpu`, `winit-core` plus platform backends, `taffy`, `cosmic-text`, `image`, `resvg`, `reqwest`, `lyon`, and optional `ffmpeg-next`.

## Workspace Layout

- `Cargo.toml`: public facade package plus `[workspace]`, shared package metadata/dependencies, root features, and publish excludes.
- `src/lib.rs`: public compatibility facade that keeps `tgui::...` API paths stable by re-exporting from `tgui-runtime`.
- `crates/tgui-runtime/`: primary implementation crate and feature owner for application/runtime/UI/rendering/media/audio/video/platform code.
- `crates/tgui-core/`: core value-type boundary, currently re-exporting canonical core types from runtime.
- `crates/tgui-platform/`: platform backend boundary.
- `crates/tgui-log/`: logging boundary.
- `crates/tgui-mvvm/`: MVVM, animation, dialog, and notification boundary.
- `crates/tgui-media/`: media loading/cache boundary.
- `crates/tgui-ui/`: layout, theme, widget, and canvas boundary.
- `crates/tgui-rendering/`: renderer boundary.
- `examples/*`: workspace example packages.
- `benches/`: workspace package `tgui-benchmarks` with explicit Criterion bench targets.
- `docs/`: VitePress documentation site; not a Cargo workspace member.

Root `cargo check` checks only `tgui` because `default-members = ["."]`. Use `cargo check --workspace --all-targets` for the full workspace.

## Key Implementation Files

- `crates/tgui-runtime/src/lib.rs`: implementation crate exports, public groups, macros, and module declarations.
- `crates/tgui-runtime/src/application/mod.rs`: `Application`, `WindowSpec`, multi-window declarations, window decoration configuration, window bindings, and run entry point.
- `crates/tgui-runtime/src/foundation/binding/mod.rs`: `ViewModelContext`, `State`, `Signal`, `TextController`, dependency tracking, invalidation.
- `crates/tgui-runtime/src/foundation/view_model/mod.rs`: `ViewModel`, `Command`, `ValueCommand`, `CommandContext`.
- `crates/tgui-runtime/src/foundation/window_control.rs`: `WindowControl`, `WindowResizeDirection`, and queued native window requests for command handlers.
- `crates/tgui-runtime/src/runtime/`: event loop integration, window lifecycle, input, focus, scrolling, text editing, commands, window control request draining, dialog callbacks, theme binding, animation refresh, media state, render scheduling.
- `crates/tgui-runtime/src/ui/layout/mod.rs`: layout value types such as `Length`, `Track`, `Insets`, `Align`, `Justify`, `Axis`, `Overflow`, `Value`.
- `crates/tgui-runtime/src/ui/widget/core/`: element tree resolution, Taffy layout, scene primitive collection, hit regions, scrolling, input/editing, selection. High-risk area.
- `crates/tgui-runtime/src/ui/widget/`: public widget builders such as button, text, input, textarea, image, checkbox, radio, select, switch, canvas, background, video.
- `crates/tgui-runtime/src/audio/`: `audio` feature API, invisible `Audio` widget, controller, metrics, and FFmpeg/CPAL backend.
- `crates/tgui-runtime/src/ui/theme/`: theme tokens, component themes, state resolution, light/dark/system mode.
- `crates/tgui-runtime/src/rendering/renderer.rs`: `wgpu` renderer and pipelines for rects, brushes, meshes, text, textures, transparent window surfaces, backdrop blur.
- `crates/tgui-runtime/src/rendering/shader/*.wgsl`: shader code.
- `crates/tgui-runtime/src/media/mod.rs`: raster image/SVG/network/memory loading, texture and shadow caches.
- `crates/tgui-runtime/src/dialog/mod.rs`: native dialogs through `rfd` on desktop.
- `crates/tgui-runtime/src/notification/mod.rs`: notifications, permissions, platform dispatch, and interactive action callbacks.
- `crates/tgui-runtime/src/platform/mod.rs`: platform abstraction and selected winit backend.
- `crates/tgui-runtime/src/video/`: `video` feature API and FFmpeg backend.
- `crates/tgui-runtime/build.rs`: Windows `video` system-library link hints.

## Features

Root features are facade forwards:

- `default = []`
- `audio`: forwards to `tgui-runtime/audio`.
- `video`: forwards to `tgui-runtime/video`.
- `video-static`: forwards to `tgui-runtime/video-static`.
- `bench-support`: forwards to `tgui-runtime/bench-support`.
- `collect-profile`: forwards to `tgui-runtime/collect-profile`.
- `mimalloc`: remains only on the public facade so downstream binary crates choose the global allocator.

`tgui-benchmarks` mirrors benchmark-related feature forwards through its dependency on `tgui`. Desktop target dependencies live in `crates/tgui-runtime/Cargo.toml`.

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
- `audio`: exported only with the `audio` feature.
- `video`: exported only with the `video` feature.
- `prelude`: convenient import set for examples and small apps.

## Runtime Flow

1. A `ViewModel` builds an `Element<VM>` tree.
2. `WidgetTree` resolves the tree and computes layout with Taffy.
3. Widgets emit scene primitives, hit regions, scroll areas, IME/caret state, and command targets.
4. `crates/tgui-runtime/src/runtime/` processes platform events, input, hover/focus/pressed state, command dispatch, window control requests, cache invalidation, media/dialog callbacks, and redraw scheduling.
5. `Renderer` submits primitives to `wgpu` pipelines.

Transparent windows are driven by clear color alpha. The renderer picks non-opaque composite alpha modes for transparent surfaces; on Windows transparent windows prefer DX12 and a DXGI visual swapchain path.

## Widget Change Checklist

- Add or update the builder API in the relevant `crates/tgui-runtime/src/ui/widget/` module.
- Store layout/visual/interaction state using existing structs where possible.
- Wire behavior into `WidgetKind`/core tree handling only where needed.
- Include hit-testing, focus, pressed/hover state, scroll behavior, text selection, IME behavior, and change-set emission when the widget participates in those systems.
- Emit scene primitives compatible with `crates/tgui-runtime/src/rendering/renderer.rs`.
- Expose public types through root `src/lib.rs` and any relevant boundary crate if the API is meant for users.
- Add focused tests near existing widget/core tests and update examples for user-facing APIs.

## Validation Targets

- Metadata: `cargo metadata --no-deps --format-version 1`.
- Facade compatibility: `cargo check -p tgui`, `cargo check -p tgui --no-default-features`, `cargo check -p tgui --features audio`, `cargo check -p tgui --features video`, `cargo check -p tgui --features video-static`.
- Full workspace: `cargo check --workspace --all-targets`.
- Runtime unit tests: `cargo test -p tgui-runtime --lib -- --test-threads=1`.
- Boundary crate tests: run package checks/tests for `tgui-core`, `tgui-mvvm`, `tgui-media`, `tgui-ui`, `tgui-rendering`, `tgui-platform`, and `tgui-log` when their exports change.
- Bench compile: `cargo bench -p tgui-benchmarks --no-run --features bench-support`; add `audio` or `video` features for those benches when relevant.
- Example smoke checks: `cargo check -p basic_window`, `cargo check -p mvvm_counter`, `cargo check -p demo`.
- Package lists: `cargo package -p tgui --allow-dirty --list` and the same for publishable internal crates when release contents matter.

Focused test locations:

- Layout, primitive, input, selection, scroll, and widget state: `crates/tgui-runtime/src/ui/widget/core/tests.rs`.
- Runtime focus, text input editing, scrollbars, command dispatch, canvas/video hit behavior: `crates/tgui-runtime/src/runtime/tests.rs`.
- Window decoration config and command window control: `crates/tgui-runtime/src/application/mod.rs` and `crates/tgui-runtime/src/foundation/window_control.rs` tests.
- Media, SVG, rasterization, external resources, caches: `crates/tgui-runtime/src/media/tests.rs`.
- Animation and timelines: `crates/tgui-runtime/src/animation/tests.rs`.
- Audio controller/backends: `crates/tgui-runtime/src/audio/controller/tests.rs`, `crates/tgui-runtime/src/audio/backend/shared/tests.rs`, `crates/tgui-runtime/src/audio/backend/ffmpeg/tests.rs`.
- Theme state and tokens: `crates/tgui-runtime/src/ui/theme/mod.rs` tests.
- Font behavior: `crates/tgui-runtime/src/text/font/tests.rs`.
- Canvas-specific behavior: `crates/tgui-runtime/src/ui/widget/canvas/tests.rs`.
- Video controller/backend: `crates/tgui-runtime/src/video/**` tests with the appropriate feature and local FFmpeg environment.

## Actual Examples To Check

Use `find examples -maxdepth 2 -name Cargo.toml` or `rg --files examples -g Cargo.toml` before editing docs because README prose can lag behind the directory. Current workspace example packages include:

- `animation_showcase`
- `background_effects`
- `basic_window`
- `canvas`
- `demo`
- `dialogs`
- `drawer_demo`
- `frameless_window`
- `list_virtual_list`
- `modal_demo`
- `multi_window`
- `multiple_vm_examples`
- `mvvm_counter`
- `table_datagrid`
- `text_area`
- `timeline_controller`
- `toast_snackbar`
- `tree`

Run examples with package names or manifest paths:

```powershell
cargo run -p basic_window
cargo run -p mvvm_counter
cargo run --manifest-path examples/canvas/Cargo.toml
cargo run --manifest-path examples/frameless_window/Cargo.toml
```

## Maintenance Notes

- Do not treat `crates/tgui-runtime/src/runtime/` or `crates/tgui-runtime/src/ui/widget/core/` as small utility modules; changes can affect input, layout, cache invalidation, rendering, commands, and platform event behavior.
- Public API changes should be checked against root `src/lib.rs` re-exports, relevant boundary crates, README/docs, examples, and tests.
- Root `Cargo.toml` excludes `docs/*`, `examples/*`, `crates/*`, `benches/*`, `AGENTS.md`, `CLAUDE.md`, `skills/*`, `Video.md`, and temporary benchmark files from the public facade package; verify package lists for release-facing changes.
- Add new platform behavior behind the existing `cfg` structure and platform abstraction.
- Text changes must respect UTF-8 boundaries, IME composition, selection ranges, caret visibility, and horizontal scrolling.
- Async media/dialog completions must trigger invalidation through the runtime.
- Audio changes should validate `audio` feature gating, controller/runtime lifecycle, and desktop playback assumptions.
- Do not delete, rename, or overwrite the untracked `Video.md` unless explicitly asked.
