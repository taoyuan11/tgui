# cargo-tgui

Native desktop bundles and installers for tgui applications. Requires Rust 1.89+.

```sh
cargo install --path crates/cargo-tgui --locked
cargo tgui doctor --manifest-path examples/packaging/Cargo.toml
cargo tgui bundle --manifest-path examples/packaging/Cargo.toml --locked
```

Configure `[package.metadata.tgui.bundle]` in the application manifest. See the
[packaging guide](https://nandebishitaoyuan.github.io/tgui/guide/packaging) for configuration,
platform prerequisites, runtime resources, FFmpeg distribution and CI usage.

Supported native targets: Windows x64 (NSIS), macOS x64/arm64 (APP/DMG), Linux x64
(DEB/AppImage). Outputs include a dependency manifest and SHA-256 checksums.
No publishing, release signing or application auto-update is performed.

The backend is pinned to cargo-packager 0.11.8. First-time NSIS/AppImage builds may
download backend tools; local DMG creation uses Apple's `hdiutil`.
