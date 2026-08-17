# Toolchain and build baseline

P0 was established on 2026-08-17 with:

- Rust stable `rustc 1.96.1 (31fca3adb 2026-06-26)` and Cargo `1.96.1`.
- MSRV: Rust 1.85, the first stable release supporting edition 2024.
- Development host: `aarch64-apple-darwin` on macOS.
- Supported release targets: Windows, macOS, and Linux.

`rust-toolchain.toml` follows the current stable channel, while `package.rust-version`
and the dedicated CI job enforce the MSRV. Backend dependencies are intentionally
absent in P0; the feature names reserve their boundaries without adding WebView,
AccessKit adapters, image/SVG codecs, or GPU libraries to the minimal core.

## Reproducible commands

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --no-default-features
cargo test
scripts/check-features.sh
scripts/ci.sh
```

The GitHub Actions workflow repeats core/default/all-feature checks on Windows,
macOS, and Linux, and runs every individual feature combination on Linux.
