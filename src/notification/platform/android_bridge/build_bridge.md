# 桥接 dex 重新生成

`bridge.dex` 是 `TguiNotificationBridge.java` 的 Android DEX 产物，运行时通过 JNI `InMemoryDexClassLoader` 加载。修改 `TguiNotificationBridge.java` 后需要重新生成。

## 环境要求

- JDK 11+（用于 `javac`）。这里以 JDK 21 为例，`javac --release 11` 限制字节码兼容到 Java 11。
- Android SDK build-tools 30+，提供 `d8`。
- Android platform jar：`platforms/android-XX/android.jar`（本地可用 34）。
- 运行时最低 API：26（取决于 `InMemoryDexClassLoader`）。

## 生成命令

在仓库根目录执行：

```bash
cd src/notification/platform/android_bridge

# 1. javac -> .class
javac --release 11 \
      -classpath "$ANDROID_SDK/platforms/android-34/android.jar" \
      -d build TguiNotificationBridge.java

# 2. d8 -> .dex
"$ANDROID_SDK/build-tools/36.0.0/d8.bat" \
      --min-api 26 \
      --output . \
      build/com/tgui/*.class

# 3. 整理
mv classes.dex bridge.dex
rm -rf build
```

Linux/macOS 把 `d8.bat` 换成 `d8`。

## 校验

修改桥接类后必须同步更新 `bridge.dex` 并提交。`cargo package --list` 必须能列出 `src/notification/platform/android_bridge/bridge.dex`，否则发布到 crates.io 后将丢失 Android 通知能力。

## 类与方法约定

Rust 侧（`mod.rs`）依赖以下符号，修改桥接类时不要改名：

- 类全名：`com.tgui.TguiNotificationBridge`
- 静态方法：
  - `install(Landroid/app/Activity;)V`
  - `sendNotification(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZJ[Ljava/lang/String;[Ljava/lang/String;)V`
  - `requestPermission(Landroid/app/Activity;J)V`
  - `permissionStatus(Landroid/app/Activity;)I`
- 静态原生回调（Rust `RegisterNatives` 时绑定，必须保持签名）：
  - `private static native void onNotificationAction(long callbackId, String actionId);`
  - `private static native void onPermissionResult(long requestId, int status);`

## 权限状态常量

`TguiNotificationBridge` 与 Rust 端的权限状态保持一致：

| Java                         | Rust |
|-----------------------------|------|
| `PERMISSION_NOT_DETERMINED` | `NotificationPermission::NotDetermined` |
| `PERMISSION_GRANTED`        | `NotificationPermission::Granted` |
| `PERMISSION_DENIED`         | `NotificationPermission::Denied` |
