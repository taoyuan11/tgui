@echo off
setlocal

echo "Publishing..."
cargo check || exit /b %ERRORLEVEL%
cargo test || exit /b %ERRORLEVEL%
cargo package --allow-dirty || exit /b %ERRORLEVEL%
cargo publish --allow-dirty || exit /b %ERRORLEVEL%
echo "Publish OK"
