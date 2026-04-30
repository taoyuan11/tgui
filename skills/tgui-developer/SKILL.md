---
name: tgui-developer
description: Build, modify, review, and document the local tgui Rust GUI framework. Use when working in the tgui repository on MVVM APIs, widgets, layout, rendering, theme tokens, animation, media loading, dialogs, native window controls, custom window chrome, optional video support, examples, Cargo features, tests, or project-specific maintenance.
---

# TGUI Developer

## Start Here

Use this skill as the project-specific operating guide for the `tgui` Rust GUI crate.

At the beginning of a task, read the local files that match the change instead of relying only on memory. Always start with `AGENTS.md`, `Cargo.toml`, `src/lib.rs`, and one nearby example or test. Load `references/project-map.md` when the task touches architecture, public API shape, runtime/widget internals, rendering, media, video, platform features, or when the best edit location is unclear.

## Workflow

1. Identify the surface area: public API, example app, widget builder, runtime behavior, renderer/shader, theme token, media/dialog/window-control/video, platform, or docs.
2. Trace from `src/lib.rs` exports to the implementation module, then to examples/tests that exercise the same API.
3. Keep edits narrow and consistent with existing builder and MVVM patterns. Prefer existing `Element`, `WidgetKind`, `LayoutStyle`, `VisualStyle`, `InteractionHandlers`, command, and invalidation paths.
4. For public API changes, update every relevant layer: implementation, re-export in `src/lib.rs`, docs/README if applicable, and at least one example or test when behavior changes.
5. Validate with the smallest meaningful command first, then broaden when the change crosses shared behavior.

## Project Patterns

- Treat `tgui` as MVVM-only: applications use `Application::new().with_view_model(...).root_view(...).run()`, and view models implement `ViewModel: Send + 'static`.
- Use `Application::decorations(false)` or `WindowSpec::decorations(false)` for custom chrome. Pair transparent frameless windows with `clear_color(Color::TRANSPARENT)` and verify renderer surface alpha behavior.
- Create reactive state through `ViewModelContext::observable`; expose UI values with `Observable::binding()` and `Binding::map`; use `Binding::animated(Transition)` only for supported interpolated property types.
- Use `Command<T>` for no-payload widget/window events and `ValueCommand<T, V>` for payload events. Use `new_with_context` when a handler needs runtime services such as dialogs or logging.
- Use `CommandContext::window()` for native window actions from commands: drag, drag-resize with `WindowResizeDirection`, minimize, maximize, restore, toggle maximize, close, and `is_maximized`.
- Preserve the chainable builder style. New bindable visual/layout properties should usually accept `impl Into<Value<T>>` so static values and bindings both work.
- Add widgets by following existing widget modules plus `src/ui/widget/core.rs`; do not introduce a parallel event, layout, hit-test, or rendering path unless the existing model cannot represent the feature.
- Treat `src/runtime.rs` and `src/ui/widget/core.rs` as high-blast-radius files. Before editing them, find the focused test helpers and add or adjust small unit tests around the exact behavior.
- For renderer or shader work, trace primitive generation in widgets first, then renderer upload/draw paths, then WGSL. Keep CPU primitive contracts and shader structs in sync.
- For window-control work, keep `ApplicationConfig`/`WindowSpec`, `CommandContext`, `src/foundation/window_control.rs`, runtime request draining, multi-window close policy, and platform window APIs aligned.
- For async media/dialog work, ensure completion returns through existing runtime/invalidation mechanisms so the UI refreshes.
- For video work, gate public exports and code paths behind `#[cfg(feature = "video")]`; remember local FFmpeg/linker setup may limit validation.

## Validation

Use the narrowest relevant checks:

```powershell
cargo fmt
cargo check
cargo test <test_name>
cargo test
cargo check --features video
cargo check --features android
cargo check --features ohos
```

Prefer module tests for `runtime.rs`, `src/ui/widget/core.rs`, `src/application/mod.rs`, `src/foundation/window_control.rs`, `src/media/mod.rs`, `src/animation.rs`, and video backend changes. Running an example is useful for smoke testing, but it is not a substitute for focused tests when shared behavior changes.

## Local Cautions

Do not delete, rename, or overwrite the untracked `Video.md` unless the user explicitly asks. Do not rely on README example names without checking `examples/`; the actual example set may differ from prose documentation. Keep platform-specific dependencies and code under the existing `cfg` structure in `Cargo.toml`, `platform.rs`, `application`, runtime, and video modules.
