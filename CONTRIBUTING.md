# Contributing to tgui

Thanks for helping improve `tgui`. The project is still in the 0.x line, so
APIs can evolve, but changes should stay intentional and easy to review.

## Before you start

- Open an issue first for large public API, runtime, layout, rendering,
  platform, notification, audio, or video changes.
- Keep PRs focused. Avoid mixing behavior changes with formatting churn.
- Respect existing workspace boundaries: `tgui` is the public facade, while
  `crates/tgui-runtime` owns most implementation details.
- Do not delete or rewrite unrelated local files. Generated build output and
  untracked local files may exist in maintainer worktrees.

## Local checks

Run the smallest useful checks while iterating, then use the broader set before
opening a PR:

```sh
cargo fmt --all -- --check
cargo check -p tgui
cargo test -p tgui-runtime --lib -- --test-threads=1
```

For shared runtime, widget core, rendering primitive, text input, notification,
media, audio, or video changes, also run the relevant focused tests:

```sh
cargo test -p tgui-runtime media
cargo test -p tgui-runtime text_input
cargo test -p tgui-runtime canvas_scene
```

Feature checks:

```sh
cargo check -p tgui --no-default-features
cargo check -p tgui --features audio
cargo check -p tgui --features video
cargo check -p tgui --features video-static
cargo check --workspace --all-targets
```

Audio/video checks need a working FFmpeg/libclang setup on the host. If that
environment is unavailable, say so in the PR and rely on CI for the full matrix.

## Public API changes

CI checks the committed public API baselines in `public-api/`. For intentional
API changes:

```sh
cargo install cargo-public-api
rustup toolchain install nightly --profile minimal
cargo +nightly public-api -p tgui --all-features --color never > public-api/tgui.txt
cargo +nightly public-api -p tgui-runtime --all-features --color never > public-api/tgui-runtime.txt
```

In the PR, classify the diff as compatible, visual-compatible, or breaking.

## Documentation

Public API changes should update the facade exports, README, docs, examples,
and migration notes when relevant. User-facing behavior changes should include
at least one example, test, or documentation note that shows the intended path.
