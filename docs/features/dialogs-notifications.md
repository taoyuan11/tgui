# 对话框与通知

对话框和通知都是运行时服务，通过 `CommandContext` 暴露给 ViewModel 命令。同步 API 会立即返回结果；异步 API 会在原生对话框或平台通知动作完成后，通过运行时 dispatcher 回到 UI 线程并执行 `ValueCommand`，因此不要在后台线程直接持有或修改 ViewModel。

## 获取运行时服务

在命令中使用 `Command::new_with_context`：

```rust
Button::new("选择文件")
    .on_click(Command::new_with_context(|vm: &mut AppVm, ctx| {
        vm.open_file(ctx);
    }))
```

```rust
impl AppVm {
    fn open_file(&mut self, ctx: &CommandContext<Self>) {
        let options = FileDialogOptions::new()
            .title("选择配置文件")
            .add_filter("Config", &["json", "toml"]);

        let _ = ctx.dialogs().open_file_async(
            options,
            ValueCommand::new(|vm: &mut AppVm, result| {
                match result {
                    Ok(Some(path)) => vm.config_path.set(path.display().to_string()),
                    Ok(None) => vm.status.set("已取消选择".to_string()),
                    Err(error) => vm.status.set(format!("打开文件失败: {error}")),
                }
            }),
        );
    }
}
```

## 文件对话框

`ctx.dialogs()` 提供文件和目录选择能力：

| API | 返回值 |
| --- | --- |
| `open_file(options)` | `Result<Option<PathBuf>, DialogError>` |
| `open_files(options)` | `Result<Option<Vec<PathBuf>>, DialogError>` |
| `pick_folder(options)` | `Result<Option<PathBuf>, DialogError>` |
| `pick_folders(options)` | `Result<Option<Vec<PathBuf>>, DialogError>` |
| `save_file(options)` | `Result<Option<PathBuf>, DialogError>` |
| `open_file_async(options, callback)` | 异步回调 `Result<Option<PathBuf>, DialogError>` |
| `open_files_async(options, callback)` | 异步回调 `Result<Option<Vec<PathBuf>>, DialogError>` |
| `pick_folder_async(options, callback)` | 异步回调 `Result<Option<PathBuf>, DialogError>` |
| `pick_folders_async(options, callback)` | 异步回调 `Result<Option<Vec<PathBuf>>, DialogError>` |
| `save_file_async(options, callback)` | 异步回调 `Result<Option<PathBuf>, DialogError>` |

`FileDialogOptions` 常用 API：

| API | 说明 |
| --- | --- |
| `title(text)` | 对话框标题。 |
| `directory(path)` | 初始目录。 |
| `file_name(name)` | 保存文件时的默认文件名。 |
| `add_filter(name, extensions)` | 文件扩展名过滤器。 |
| `can_create_directories(bool)` | 保存或选目录时是否允许创建目录。 |

保存文件示例：

```rust
let options = FileDialogOptions::new()
    .title("导出报告")
    .file_name("report.json")
    .add_filter("JSON", &["json"])
    .can_create_directories(true);

let _ = ctx.dialogs().save_file_async(
    options,
    ValueCommand::new(|vm: &mut AppVm, result| {
        if let Ok(Some(path)) = result {
            vm.export_to(path);
        }
    }),
);
```

同步 API 适合非常短的命令流程；桌面 UI 中更推荐异步 API，避免阻塞事件循环。

## 消息对话框

消息对话框用于调用平台原生确认框、警告框或错误提示。

```rust
let options = MessageDialogOptions::new()
    .title("确认覆盖")
    .description("目标文件已存在，是否覆盖？")
    .level(MessageDialogLevel::Warning)
    .buttons(MessageDialogButtons::YesNo);

let _ = ctx.dialogs().show_message_async(
    options,
    ValueCommand::new(|vm: &mut AppVm, result| {
        if matches!(result, Ok(MessageDialogResult::Yes)) {
            vm.confirm_overwrite();
        }
    }),
);
```

相关枚举：

| 类型 | 值 |
| --- | --- |
| `MessageDialogLevel` | `Info`、`Warning`、`Error` |
| `MessageDialogButtons` | `Ok`、`OkCancel`、`YesNo`、`YesNoCancel` |
| `MessageDialogResult` | `Yes`、`No`、`Ok`、`Cancel` |

应用内自定义确认流程优先使用 `Modal`；需要操作系统原生视觉或系统级确认时使用消息对话框。

## 系统通知

`ctx.notifications()` 提供系统通知发送、权限查询和交互动作回调。

| API | 说明 |
| --- | --- |
| `send(options)` | 发送普通系统通知，返回通知 ID。 |
| `send_with_actions(options, callback)` | 发送带动作按钮的通知，用户点击动作后回调。 |
| `request_permission(callback)` | 请求通知权限。 |
| `permission_status()` | 查询当前权限状态。 |

普通通知：

```rust
let result = ctx.notifications().send(
    NotificationOptions::new("导出完成")
        .body("报告已保存到 Downloads")
        .app_name("tgui demo")
        .sound(true),
);

if let Err(error) = result {
    self.status.set(format!("通知发送失败: {error}"));
}
```

交互式通知最多支持两个动作：

```rust
let options = NotificationOptions::new("构建完成")
    .body("是否打开输出目录？")
    .action(NotificationAction::new("open", "打开"))
    .action(NotificationAction::new("dismiss", "忽略"));

let _ = ctx.notifications().send_with_actions(
    options,
    ValueCommand::new(|vm: &mut AppVm, result| {
        match result {
            Ok(event) if event.action_id == "open" => vm.open_output_dir(),
            Ok(_) => {}
            Err(error) => vm.status.set(format!("通知动作失败: {error}")),
        }
    }),
);
```

`NotificationOptions` 常用 API：

| API | 说明 |
| --- | --- |
| `new(title)` | 创建通知；title 不能为空白字符串。 |
| `id(value)` | 设置稳定通知 ID；未设置时运行时自动生成。 |
| `body(text)` | 正文。 |
| `subtitle(text)` | 副标题。 |
| `app_name(text)` | 平台通知中心显示的应用名。 |
| `icon(name)` | 平台后端可识别的图标名或资源标识。 |
| `sound(bool)` | 是否播放提示音。 |
| `action(...)` / `actions(...)` | 追加交互动作。 |

权限状态：

```rust
match ctx.notifications().permission_status() {
    Ok(NotificationPermission::Granted) => self.can_notify.set(true),
    Ok(NotificationPermission::Denied) => self.can_notify.set(false),
    Ok(NotificationPermission::NotDetermined) => {
        let _ = ctx.notifications().request_permission(ValueCommand::new(
            |vm: &mut AppVm, result| {
                vm.can_notify.set(matches!(result, Ok(NotificationPermission::Granted)));
            },
        ));
    }
    Err(error) => self.status.set(format!("通知不可用: {error}")),
}
```

## 平台注意事项

- Windows：建议设置稳定的 `Application::app_id(...)`，这是通知身份初始化的重要前置条件。
- Linux：通知通过 `notify-rust` 发送并监听 action。
- macOS：接口已公开，具体能力取决于 UserNotifications 后端状态。
- 异步回调会回到运行时主线程，但原生对话框和系统通知的可用性仍取决于目标桌面环境。
- 对话框和通知服务只在命令上下文中可用；不要在 ViewModel 构造阶段直接调用。
