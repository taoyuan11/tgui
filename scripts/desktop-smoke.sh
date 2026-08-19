#!/usr/bin/env sh
set -eu

# Opens a real platform window, configures a wgpu surface, submits one frame,
# and exits. This command requires an interactive desktop session.
TGUI_SMOKE_ONCE=1 \
TGUI_SMOKE_RESIZE=1 \
TGUI_SMOKE_DEVICE_LOSS=1 \
cargo run --example p7_desktop --no-default-features --features desktop
