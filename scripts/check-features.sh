#!/usr/bin/env sh
set -eu

run_check() {
    description="$1"
    shift
    echo "checking ${description}"
    cargo check --all-targets "$@"
}

run_check "minimal core" --no-default-features
run_check "default features"
run_check "desktop" --no-default-features --features desktop
run_check "render" --no-default-features --features render
run_check "text" --no-default-features --features text
run_check "image" --no-default-features --features image
run_check "svg" --no-default-features --features svg
run_check "accessibility" --no-default-features --features accessibility
run_check "webview" --no-default-features --features webview
run_check "all features" --all-features
