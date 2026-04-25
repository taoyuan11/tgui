use std::path::PathBuf;

use tgui::{
    Application, Axis, Button, Command, DialogError, Element, FileDialogOptions, Flex,
    MessageDialogButtons, MessageDialogLevel, MessageDialogOptions, MessageDialogResult,
    Observable, Text, ValueCommand, ViewModelContext, el, pct,
};

struct App {
    clicks: Observable<u32>,
    file_status: Observable<String>,
    message_status: Observable<String>,
}

impl App {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            clicks: context.observable(0),
            file_status: context.observable("尚未选择文件".to_string()),
            message_status: context.observable("尚未显示消息框".to_string()),
        }
    }

    fn open_file_sync(&mut self, ctx: &tgui::CommandContext<Self>) {
        let result = ctx.dialogs().open_file(
            FileDialogOptions::new()
                .title("选择一个文本文件")
                .directory("/")
                .add_filter("文本文件", &["txt", "md"]),
        );
        self.file_status.set(Self::describe_file_result(result));
    }

    fn open_file_async(ctx: &tgui::CommandContext<Self>) {
        let _ = ctx.dialogs().open_file_async(
            FileDialogOptions::new()
                .title("异步选择一个文本文件")
                .add_filter("文本文件", &["txt", "md"]),
            ValueCommand::new(Self::apply_async_file_result),
        );
    }

    fn show_message_sync(&mut self, ctx: &tgui::CommandContext<Self>) {
        let result = ctx.dialogs().show_message(
            MessageDialogOptions::new()
                .title("确认")
                .description("是否将状态切换为“已确认”？")
                .level(MessageDialogLevel::Warning)
                .buttons(MessageDialogButtons::YesNo),
        );
        self.message_status
            .set(Self::describe_message_result(result));
    }

    fn show_message_async(ctx: &tgui::CommandContext<Self>) {
        let _ = ctx.dialogs().show_message_async(
            MessageDialogOptions::new()
                .title("异步提示")
                .description("这是一个异步消息框示例。")
                .level(MessageDialogLevel::Info)
                .buttons(MessageDialogButtons::OkCancel),
            ValueCommand::new(Self::apply_async_message_result),
        );
    }

    fn increment(&mut self) {
        self.clicks.update(|clicks| *clicks += 1);
    }

    fn apply_async_file_result(&mut self, result: Result<Option<PathBuf>, DialogError>) {
        self.file_status
            .set(format!("异步结果: {}", Self::describe_file_result(result)));
    }

    fn apply_async_message_result(&mut self, result: Result<MessageDialogResult, DialogError>) {
        self.message_status.set(format!(
            "异步结果: {}",
            Self::describe_message_result(result)
        ));
    }

    fn describe_file_result(result: Result<Option<PathBuf>, DialogError>) -> String {
        match result {
            Ok(Some(path)) => format!("已选择: {}", path.display()),
            Ok(None) => "已取消选择".to_string(),
            Err(error) => format!("打开失败: {error}"),
        }
    }

    fn describe_message_result(result: Result<MessageDialogResult, DialogError>) -> String {
        match result {
            Ok(choice) => format!("按钮结果: {choice:?}"),
            Err(error) => format!("消息框失败: {error}"),
        }
    }

    fn view(&self) -> Element<Self> {
        let open_sync = Button::new(Text::new("同步文件选择")).on_click(Command::new_with_context(
            |app: &mut App, ctx| app.open_file_sync(ctx),
        ));

        let open_async = Button::new(Text::new("异步文件选择")).on_click(
            Command::new_with_context(|_: &mut App, ctx| Self::open_file_async(ctx)),
        );

        let message_sync = Button::new(Text::new("同步确认框")).on_click(
            Command::new_with_context(|app: &mut App, ctx| app.show_message_sync(ctx)),
        );

        let message_async = Button::new(Text::new("异步消息框")).on_click(
            Command::new_with_context(|_: &mut App, ctx| Self::show_message_async(ctx)),
        );

        let counter = Button::new(Text::new(
            self.clicks
                .binding()
                .map(|clicks| format!("普通按钮，点击了 {clicks} 次")),
        ))
        .on_click(Command::new(Self::increment));

        Flex::new(Axis::Vertical)
            .size(pct(100.0), pct(100.0))
            .child(el![
                open_sync,
                open_async,
                message_sync,
                message_async,
                counter,
                Text::new(self.file_status.binding()),
                Text::new(self.message_status.binding()),
            ])
            .into()
    }
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .with_view_model(App::new)
        .root_view(App::view)
        .run()
}
