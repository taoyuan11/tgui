## x86-64 run
```shell
$env:ANDROID_HOME="D:\DeveloperComponents\Android\SDK"

$env:ANDROID_NDK_ROOT="D:\DeveloperComponents\Android\SDK\ndk\30.0.14904198"

cargo apk build --manifest-path examples/android_basic_window/Cargo.toml --target x86_64-linux-android
```

## arm64 run
```shell

$env:ANDROID_HOME="D:\DeveloperComponents\Android\SDK"

$env:ANDROID_NDK_ROOT="D:\DeveloperComponents\Android\SDK\ndk\30.0.14904198"

cargo apk build --manifest-path examples/android_basic_window/Cargo.toml --target aarch64-linux-android
```