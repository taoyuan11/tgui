use std::fs;
use std::path::Path;
use tgui::prelude::*;

fn card_style(context: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.border_color = Some(Color::rgb(48, 58, 76).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(14.0).into());
    style
}

fn title_style(context: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(context.theme);
    style.typography.size = sp(26.0);
    style.typography.weight = FontWeight::Medium;
    style
}

struct App {
    theme: State<ThemeMode>,
    content: TextController,
    path_label: State<String>,
}

impl App {
    fn theme_binding(&self) -> Signal<ThemeMode> {
        self.theme.signal()
    }

    fn get_source(path: &str) -> String {
        let path = Path::new(path);
        let content = fs::read_to_string(&path).unwrap_or_else(|error| {
            format!("无法读取示例源码:\n{error}\n\n目标文件: {}", path.display())
        });
        content
    }
}

impl ViewModel for App {
    fn new(context: &ViewModelContext) -> Self {
        let source_path =
            String::from("/Users/sky/Desktop/Project/Rust/libs/tgui/src/runtime/mod.rs");

        let source = Self::get_source(source_path.as_str());

        Self {
            theme: context.state(ThemeMode::System),
            content: context.text_controller(source),
            path_label: context.state(source_path.clone()),
        }
    }

    fn view(&self) -> Element<Self> {
        Flex::vertical()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(20.0)))
            .gap(dp(12.0))
            .on_update(Command::new(|_| {
                tgui_log(LogLevel::Info, "root view update")
            }))
            .child(el![
                Text::new("Textarea 示例").style_full(title_style),
                Text::new(
                    "下面的内容读取自当前示例的 `main.rs`，你可以编辑它，但修改不会保存到磁盘。"
                )
                .user_select(true),
                Text::new(self.path_label.signal()).user_select(true),
                Flex::vertical()
                    .padding(Insets::all(dp(14.0)))
                    .gap(dp(10.0))
                    .width(pct(100.0))
                    .min_height(dp(0.0))
                    .grow(1.0)
                    .style_full(card_style)
                    .child(el![
                        Text::new("源码编辑区"),
                        Textarea::new(self.content.clone())
                            .width(pct(100.0))
                            .min_height(dp(0.0))
                            .grow(1.0)
                    ]),
            ])
            .into()
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .title("tgui textarea example")
        .window_size(dp(960.0), dp(640.0))
        .with_view_model(App::new)
        .root_view(App::view)
        .bind_theme_mode(App::theme_binding)
        .run()
}
