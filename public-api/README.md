# Public API baselines

This directory stores the checked-in public API surface for the published
facade crate and the runtime implementation crate.

CI regenerates both files with `cargo-public-api` and fails if the output
differs from the committed baselines. For intentional API changes, review the
diff in the pull request and then refresh the matching baseline:

```sh
cargo install cargo-public-api
rustup toolchain install nightly --profile minimal
cargo +nightly public-api -p tgui --all-features --color never > public-api/tgui.txt
cargo +nightly public-api -p tgui-runtime --all-features --color never > public-api/tgui-runtime.txt
```

When updating these files, include a short PR note that classifies the change
as compatible, visual-compatible, or breaking. The 0.x line can still evolve,
but API drift should be explicit.
