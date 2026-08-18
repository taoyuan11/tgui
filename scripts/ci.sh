#!/usr/bin/env sh
set -eu

cargo fmt --all -- --check
cargo clippy --all-targets --no-default-features -- -D warnings
cargo clippy --all-targets --all-features -- -D warnings
scripts/check-features.sh
cargo test --no-default-features
cargo test
cargo run --example p0_headless --no-default-features
cargo run --example p1_headless --no-default-features
