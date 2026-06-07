# 对话框与通知

运行时服务通过 `CommandContext` 暴露给 ViewModel 命令。对话框和通知回调会回到运行时调度链路，不需要在后台线程直接修改 ViewModel。

## 对话框

`ctx.dialogs()` 提供原生文件选择和消息框能力，桌面端由平台后端实现。

适合场景：

- 打开或保存文件。
- 选择目录。
- 显示确认、警告或错误消息。

异步对话框完成后会通过 runtime dispatcher 回到 ViewModel。

## 系统通知

`ctx.notifications()` 提供：

- `send`：发送普通系统通知。
- `send_with_actions`：发送最多两个 action 的交互式通知。
- `request_permission`：请求通知权限。
- `permission_status`：查询权限状态。

`NotificationOptions` 支持 title、body、subtitle、app name、icon、声音开关和 action。

## 平台注意事项

- Windows：建议设置稳定的 `Application::app_id(...)`，这是通知身份初始化的重要前置条件。
- Linux：通过 `notify-rust` 发送通知并监听 action。
- macOS：接口已公开，具体能力取决于 UserNotifications 后端状态。
