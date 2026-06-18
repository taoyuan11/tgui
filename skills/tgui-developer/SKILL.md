---
name: tgui-developer
description: Build, modify, review, and document the local multi-crate tgui Rust GUI workspace. Use when working in the tgui repository on the public facade, internal crates, MVVM APIs, widgets, layout, rendering, theme tokens, animation, text input, notifications, media loading, dialogs, native window controls, custom window chrome, optional audio/video support, examples, Cargo features, tests, or project-specific maintenance.
---

# TGUI Developer

## Start Here

Use this skill as the project-specific operating guide for the `tgui` Rust GUI workspace.

At the beginning of a task, read local files that match the change instead of relying only on memory. Always start with `AGENTS.md`, root `Cargo.toml`, root `src/lib.rs`, `crates/tgui-runtime/Cargo.toml`, and one nearby example or test. Load `references/project-map.md` when the task touches architecture, public API shape, runtime/widget internals, rendering, media, notifications, video, platform features, workspace packaging, or when the best edit location is unclear.

For text selection, caret, IME, `Input`, or `Textarea` work, read `crates/tgui-runtime/src/ui/widget/input/mod.rs`, `crates/tgui-runtime/src/ui/widget/textarea/mod.rs`, `crates/tgui-runtime/src/ui/widget/common.rs`, `crates/tgui-runtime/src/ui/widget/core/`, `crates/tgui-runtime/src/runtime/input/`, and `crates/tgui-runtime/src/runtime/mod.rs`.

For notification work, read `crates/tgui-runtime/src/notification/mod.rs`, `crates/tgui-runtime/src/application/mod.rs`, `crates/tgui-runtime/src/foundation/view_model/mod.rs`, and `examples/demo/src/main.rs`.

For audio or video work, read `crates/tgui-runtime/src/audio/` or `crates/tgui-runtime/src/video/` first, then trace the feature-gated facade export in root `src/lib.rs`, one example if available, and the related runtime tests.

## Workflow

1. Identify the surface area: public facade, internal crate boundary, example app, widget builder, runtime behavior, text input, renderer/shader, theme token, notifications, media/dialog/window-control/audio/video, platform, docs, or packaging.
2. Trace from root `src/lib.rs` facade exports to `crates/tgui-runtime/src/lib.rs`, then to the implementation module and examples/tests that exercise the same API.
3. Keep edits narrow and consistent with existing builder and MVVM patterns. Prefer existing `Element`, `WidgetKind`, `LayoutStyle`, `VisualStyle`, `InteractionHandlers`, command, text-controller, and invalidation paths.
4. For public API changes, update every relevant layer: implementation, facade re-export in root `src/lib.rs`, boundary crate export if applicable, docs/README, and at least one example or test when behavior changes.
5. Validate with the smallest meaningful package-qualified command first, then broaden when the change crosses shared behavior.

## Project Patterns

- Treat public `tgui` as a compatibility facade. Most implementation lives in `crates/tgui-runtime`; boundary crates under `crates/tgui-*` expose publishable compile/API surfaces.
- Treat applications as MVVM-first: `Application::new().with_view_model(...).root_view(...).run()`, and view models implement `ViewModel: Send + 'static`.
- Use `Application::decorations(false)` or `WindowSpec::decorations(false)` for custom chrome. Pair transparent frameless windows with `clear_color(Color::TRANSPARENT)` and verify renderer surface alpha behavior.
- Create reactive state through `ViewModelContext::state`; expose UI values with `State::signal()` and `Signal::map`; use `Signal::animated(Transition)` only for supported interpolated property types.
- Use `TextController` for retained `Input` and `Textarea` state, and follow `TextChangeSet` plus runtime invalidation when text editing behavior changes.
- Use `Command<T>` for no-payload widget/window events and `ValueCommand<T, V>` for payload events. Use `new_with_context` when a handler needs runtime services such as dialogs, notifications, window control, or logging.
- Use `CommandContext::notifications()` for system notifications and interactive notification actions. When documenting or implementing notification flows, verify whether `Application::app_id(...)` is required on the target platform.
- Use `CommandContext::window()` for native window actions from commands: drag, drag-resize with `WindowResizeDirection`, minimize, maximize, restore, toggle maximize, close, and `is_maximized`.
- Use the `audio` feature for playback APIs. `Audio` is an invisible widget-tree node, while `AudioController` owns playback state and commands.
- Preserve the chainable builder style. New bindable visual/layout properties should usually accept `impl Into<Value<T>>` so static values and `Signal<T>` both work.
- Add widgets by following existing widget modules plus `crates/tgui-runtime/src/ui/widget/core/`; do not introduce a parallel event, layout, hit-test, or rendering path unless the existing model cannot represent the feature.
- Treat `crates/tgui-runtime/src/runtime/` and `crates/tgui-runtime/src/ui/widget/core/` as high-blast-radius areas. Before editing them, find the focused test helpers and add or adjust small unit tests around the exact behavior.
- For renderer or shader work, trace primitive generation in widgets first, then renderer upload/draw paths, then WGSL. Keep CPU primitive contracts and shader structs in sync.
- For window-control work, keep `ApplicationConfig`/`WindowSpec`, `CommandContext`, `crates/tgui-runtime/src/foundation/window_control.rs`, runtime request draining, multi-window close policy, and platform window APIs aligned.
- For async media/dialog work, ensure completion returns through existing runtime/invalidation mechanisms so the UI refreshes.
- For video work, gate public exports and code paths behind `#[cfg(feature = "video")]`; remember local FFmpeg/linker setup may limit validation.

## Validation

Use the narrowest relevant checks:

```powershell
cargo fmt
cargo metadata --no-deps --format-version 1
cargo check -p tgui
cargo check -p tgui --no-default-features
cargo check -p tgui --features audio
cargo check -p tgui --features video
cargo check -p tgui --features video-static
cargo test -p tgui-runtime --lib -- --test-threads=1
cargo check --workspace --all-targets
```

Prefer module tests in `crates/tgui-runtime/src/runtime/tests.rs`, `crates/tgui-runtime/src/ui/widget/core/tests.rs`, `crates/tgui-runtime/src/application/mod.rs`, `crates/tgui-runtime/src/foundation/window_control.rs`, `crates/tgui-runtime/src/notification/tests.rs`, `crates/tgui-runtime/src/media/tests.rs`, `crates/tgui-runtime/src/animation/tests.rs`, audio backend/controller changes, and video backend changes. Running an example is useful for smoke testing, but it is not a substitute for focused tests when shared behavior changes.

## Local Cautions

Do not delete, rename, or overwrite the untracked `Video.md` unless the user explicitly asks. Do not rely on README example names without checking `examples/`; examples are workspace members and the actual set may differ from prose documentation. Root `cargo check` checks only the facade `tgui` because `default-members = ["."]`; use `--workspace` when validating every package. Keep desktop platform-specific dependencies and code under the existing `cfg` structure in `crates/tgui-runtime/Cargo.toml`, `platform`, `application`, runtime, notification, and video modules. Remember that the root package excludes `AGENTS.md`, `CLAUDE.md`, `docs/*`, `examples/*`, `crates/*`, `benches/*`, and `skills/*` from the published facade crate, so release-facing documentation changes may also need README or docs updates elsewhere.
