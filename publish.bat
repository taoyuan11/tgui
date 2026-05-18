@echo off
setlocal EnableDelayedExpansion

echo "Publishing..."

rem 1. 工作区清洁度：默认要求 git status 干净，可通过 PUBLISH_ALLOW_DIRTY=1 显式跳过。
if /I not "%PUBLISH_ALLOW_DIRTY%"=="1" (
    for /f "delims=" %%i in ('git status --porcelain') do (
        echo Working tree is dirty. Commit or stash before publishing.
        echo Set PUBLISH_ALLOW_DIRTY=1 to bypass intentionally.
        git status --short
        exit /b 1
    )
)

rem 2. 校验 Cargo.toml 的 version 与当前 HEAD 的 git tag 对齐。
for /f "tokens=2 delims== " %%v in ('findstr /R "^version *= *\"" Cargo.toml') do set CRATE_VERSION=%%~v
if not defined CRATE_VERSION (
    echo Failed to read version from Cargo.toml
    exit /b 1
)
set CRATE_VERSION=%CRATE_VERSION:"=%
echo Crate version: !CRATE_VERSION!

if /I not "%PUBLISH_ALLOW_UNTAGGED%"=="1" (
    git rev-parse "v!CRATE_VERSION!" >nul 2>&1
    if errorlevel 1 (
        echo Tag v!CRATE_VERSION! not found. Create it before publishing:
        echo     git tag -a v!CRATE_VERSION! -m "v!CRATE_VERSION!"
        echo Set PUBLISH_ALLOW_UNTAGGED=1 to bypass intentionally.
        exit /b 1
    )

    for /f "delims=" %%h in ('git rev-list -n 1 "v!CRATE_VERSION!"') do set TAG_COMMIT=%%h
    for /f "delims=" %%h in ('git rev-parse HEAD') do set HEAD_COMMIT=%%h
    if not "!TAG_COMMIT!"=="!HEAD_COMMIT!" (
        echo HEAD ^(!HEAD_COMMIT!^) does not match tag v!CRATE_VERSION! ^(!TAG_COMMIT!^).
        echo Move or recreate the tag before publishing.
        exit /b 1
    )
)

rem 3. 标准检查链。
cargo fmt --check || exit /b %ERRORLEVEL%
cargo check || exit /b %ERRORLEVEL%
cargo test || exit /b %ERRORLEVEL%

rem 4. 打包与发布。除非显式 opt-in，否则不再使用 --allow-dirty。
set PACKAGE_FLAGS=
set PUBLISH_FLAGS=
if /I "%PUBLISH_ALLOW_DIRTY%"=="1" (
    set PACKAGE_FLAGS=--allow-dirty
    set PUBLISH_FLAGS=--allow-dirty
)

cargo package %PACKAGE_FLAGS% || exit /b %ERRORLEVEL%
cargo publish %PUBLISH_FLAGS% || exit /b %ERRORLEVEL%

echo "Publish OK"
