## x86-64 run
```shell
$env:ANDROID_HOME="D:\DeveloperComponents\Android\SDK"

$env:ANDROID_NDK_ROOT="D:\DeveloperComponents\Android\SDK\ndk\30.0.14904198"

cargo apk build --manifest-path examples/android_basic_window/Cargo.toml --target x86_64-linux-android

adb install -r D:\Project\Rust\libs\tgui\examples\android_basic_window\target\debug\apk\android_basic_window.apk
```

## arm64 run
```shell

$env:ANDROID_HOME="D:\DeveloperComponents\Android\SDK"

$env:ANDROID_NDK_ROOT="D:\DeveloperComponents\Android\SDK\ndk\30.0.14904198"

cargo apk build --manifest-path examples/android_basic_window/Cargo.toml --target aarch64-linux-android
```

## Notification test

- 当前示例已加入 Android 通知测试区。
- Android 13+ 请先点击“请求通知权限”。
- 然后可分别测试“发送普通通知”和“发送动作通知”。
- 动作通知的回调只保证在应用进程仍然存活时回到 ViewModel。
