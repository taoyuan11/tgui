---
name: tgui-developer
description: Build, modify, review, and document the local tgui Rust GUI framework. Use when working in the tgui repository on MVVM APIs, widgets, layout, rendering, theme tokens, animation, text input, notifications, media loading, dialogs, native window controls, custom window chrome, optional audio/video support, examples, Cargo features, tests, or project-specific maintenance.
---

# TGUI Developer

## Start Here

Use this skill as the project-specific operating guide for the `tgui` Rust GUI crate.

At the beginning of a task, read the local files that match the change instead of relying only on memory. Always start with `AGENTS.md`, `Cargo.toml`, `src/lib.rs`, and one nearby example or test. Load `references/project-map.md` when the task touches architecture, public API shape, runtime/widget internals, rendering, media, notifications, video, platform features, or when the best edit location is unclear.

For text selection, caret, IME, `Input`, or `Textarea` work, read `src/ui/widget/input/mod.rs`, `src/ui/widget/textarea/mod.rs`, `src/ui/widget/common.rs`, `src/ui/widget/core/`, `src/runtime/input/`, and `src/runtime/mod.rs`.

For notification work, read `src/notification/mod.rs`, `src/application/mod.rs`, `src/foundation/view_model/mod.rs`, and `examples/demo/src/main.rs`.

For audio or video work, read `src/audio/` or `src/video/` first, then trace the corresponding widget export in `src/lib.rs`, one example if available, and the related runtime tests.

## Workflow

1. Identify the surface area: public API, example app, widget builder, runtime behavior, text input, renderer/shader, theme token, notifications, media/dialog/window-control/audio/video, platform, or docs.
2. Trace from `src/lib.rs` exports to the implementation module, then to examples/tests that exercise the same API.
3. Keep edits narrow and consistent with existing builder and MVVM patterns. Prefer existing `Element`, `WidgetKind`, `LayoutStyle`, `VisualStyle`, `InteractionHandlers`, command, text-controller, and invalidation paths.
4. For public API changes, update every relevant layer: implementation, re-export in `src/lib.rs`, docs/README if applicable, and at least one example or test when behavior changes.
5. Validate with the smallest meaningful command first, then broaden when the change crosses shared behavior.

## Project Patterns

- Treat `tgui` as MVVM-only: applications use `Application::new().with_view_model(...).root_view(...).run()`, and view models implement `ViewModel: Send + 'static`.
- Use `Application::decorations(false)` or `WindowSpec::decorations(false)` for custom chrome. Pair transparent frameless windows with `clear_color(Color::TRANSPARENT)` and verify renderer surface alpha behavior.
- Create reactive state through `ViewModelContext::state`; expose UI values with `State::signal()` and `Signal::map`; use `Signal::animated(Transition)` only for supported interpolated property types.
- Use `TextController` for retained `Input` and `Textarea` state, and follow `TextChangeSet` plus runtime invalidation when text editing behavior changes.
- Use `Command<T>` for no-payload widget/window events and `ValueCommand<T, V>` for payload events. Use `new_with_context` when a handler needs runtime services such as dialogs, notifications, window control, or logging.
- Use `CommandContext::notifications()` for system notifications and interactive notification actions. When documenting or implementing notification flows, verify whether `Application::app_id(...)` is required on the target platform.
- Use `CommandContext::window()` for native window actions from commands: drag, drag-resize with `WindowResizeDirection`, minimize, maximize, restore, toggle maximize, close, and `is_maximized`.
- Use the `audio` feature for playback APIs. `Audio` is an invisible widget-tree node, while `AudioController` owns playback state and commands.
- Preserve the chainable builder style. New bindable visual/layout properties should usually accept `impl Into<Value<T>>` so static values and `Signal<T>` both work.
- Public `Input` and `Textarea` widgets exist. Treat text-editing changes as shared selection, caret, IME, scroll, and change-set infrastructure rather than isolated widget-local behavior.
- Add widgets by following existing widget modules plus `src/ui/widget/core/`; do not introduce a parallel event, layout, hit-test, or rendering path unless the existing model cannot represent the feature.
- Treat `src/runtime/` and `src/ui/widget/core/` as high-blast-radius areas. Before editing them, find the focused test helpers and add or adjust small unit tests around the exact behavior.
- For renderer or shader work, trace primitive generation in widgets first, then renderer upload/draw paths, then WGSL. Keep CPU primitive contracts and shader structs in sync.
- For window-control work, keep `ApplicationConfig`/`WindowSpec`, `CommandContext`, `src/foundation/window_control.rs`, runtime request draining, multi-window close policy, and platform window APIs aligned.
- For async media/dialog work, ensure completion returns through existing runtime/invalidation mechanisms so the UI refreshes.
- For audio work, validate both controller behavior and at least one realistic load/playback path when the environment allows it.
- For video work, gate public exports and code paths behind `#[cfg(feature = "video")]`; remember local FFmpeg/linker setup may limit validation.

## Validation

Use the narrowest relevant checks:

```powershell
cargo fmt
cargo check
cargo test <test_name>
cargo test
cargo check --features audio
cargo check --features video
cargo check --features video-static
```

Prefer module tests for `src/runtime/tests.rs`, `src/ui/widget/core/tests.rs`, `src/application/mod.rs`, `src/foundation/window_control.rs`, `src/notification/tests.rs`, `src/media/tests.rs`, `src/animation/tests.rs`, audio backend/controller changes, and video backend changes. Running an example is useful for smoke testing, but it is not a substitute for focused tests when shared behavior changes.

## Local Cautions

Do not delete, rename, or overwrite the untracked `Video.md` unless the user explicitly asks. Do not rely on README example names without checking `examples/`; the actual example set may differ from prose documentation. Keep desktop platform-specific dependencies and code under the existing `cfg` structure in `Cargo.toml`, `platform.rs`, `application`, runtime, notification, and video modules. Remember that `Cargo.toml` excludes `AGENTS.md` and `skills/*` from publishing, so release-facing documentation changes may also need README or docs updates elsewhere.
