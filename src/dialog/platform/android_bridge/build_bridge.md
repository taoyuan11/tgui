# 桥接 dex 重新生成

`bridge.dex` 是 `TguiDialogBridge.java` 的 Android DEX 产物，运行时通过 JNI `InMemoryDexClassLoader` 加载。修改 `TguiDialogBridge.java` 后需要重新生成。

## 环境要求

- JDK 11+（用于 `javac`）。这里以 JDK 21 为例，`javac --release 11` 限制字节码兼容到 Java 11。
- Android SDK build-tools 30+，提供 `d8`。
- Android platform jar：`platforms/android-XX/android.jar`（API 26+ 即可，本地用 34）。
- 运行时最低 API：26（取决于 `InMemoryDexClassLoader`）。

## 生成命令

在仓库根目录执行（按需替换 SDK 路径）：

```bash
cd src/dialog/platform/android_bridge

# 1. javac → .class
javac --release 11 \
      -classpath "$ANDROID_SDK/platforms/android-34/android.jar" \
      -d build TguiDialogBridge.java

# 2. d8 → .dex
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

`bridge.dex` 应在 5–15 KB 之间。修改桥接类后必须同步更新该文件并提交 git。`cargo package --list` 必须能列出 `src/dialog/platform/android_bridge/bridge.dex`，否则发布到 crates.io 后将丢失 Android 对话框能力。

## 类与方法约定

Rust 侧（`mod.rs`）依赖以下符号，修改桥接类时不要改名：

- 类全名：`com.tgui.TguiDialogBridge`
- 静态方法：
  - `showMessageDialog(Landroid/app/Activity;JLjava/lang/String;Ljava/lang/String;I)V`
  - `startFileDialog(Landroid/app/Activity;JILjava/lang/String;Ljava/lang/String;[Ljava/lang/String;)V`
- 静态原生回调（Rust `RegisterNatives` 时绑定，必须保持签名）：
  - `private static native void onMessageResult(long requestId, int which);`
  - `private static native void onFileResult(long requestId, int resultCode, String[] uris);`
- 内部 `ResultFragment` 类不需要被 Rust 直接引用，但必须存在以承载 `onActivityResult`。

## 桥接结果常量

`TguiDialogBridge` 内的常量与 Rust 端保持一致：

| Java                       | 含义                            |
|----------------------------|---------------------------------|
| `BUTTON_OK = 1`            | `MessageDialogResult::Ok`       |
| `BUTTON_CANCEL = 2`        | `MessageDialogResult::Cancel`   |
| `BUTTON_YES = 3`           | `MessageDialogResult::Yes`      |
| `BUTTON_NO = 4`            | `MessageDialogResult::No`       |
| `BUTTONS_OK = 0`           | `MessageDialogButtons::Ok`      |
| `BUTTONS_OK_CANCEL = 1`    | `MessageDialogButtons::OkCancel`|
| `BUTTONS_YES_NO = 2`       | `MessageDialogButtons::YesNo`   |
| `BUTTONS_YES_NO_CANCEL = 3`| `MessageDialogButtons::YesNoCancel`|
| `FILE_OPEN = 0`            | `FileDialogRequest::OpenFile`   |
| `FILE_OPEN_MULTI = 1`      | `FileDialogRequest::OpenFiles`  |
| `FILE_PICK_FOLDER = 2`     | `FileDialogRequest::PickFolder` / `PickFolders` |
| `FILE_SAVE = 3`            | `FileDialogRequest::SaveFile`   |

`PickFolders`（多目录）在 Android SAF 上没有原生支持，与 `PickFolder` 共用同一个 intent，返回单个 tree URI。
