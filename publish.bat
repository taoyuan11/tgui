@echo off

echo "Publishing..."
cargo check
cargo test
cargo package --allow-dirty
cargo publish --allow-dirty
echo "Publish OK"