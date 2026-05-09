use std::fs;
use std::path::PathBuf;
use tgui::prelude::*;

fn card_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.border_color = Some(Color::rgb(48, 58, 76).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(14.0).into());
    style
}

fn title_style(mode: ResolvedThemeMode) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.typography.size = sp(26.0);
    style.typography.weight = FontWeight::Medium;
    style
}

struct App {
    content: TextController,
    path_label: Observable<String>,
}

impl ViewModel for App {
    fn new(context: &ViewModelContext) -> Self {
        let path = source_path();
        let content = fs::read_to_string(&path).unwrap_or_else(|error| {
            format!("无法读取示例源码:\n{error}\n\n目标文件: {}", path.display())
        });

        Self {
            content: context.text_controller(content),
            path_label: context.observable(path.display().to_string()),
        }
    }

    fn view(&self) -> Element<Self> {
        Flex::vertical()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(20.0)))
            .gap(dp(12.0))
            .child(el![
                Text::new("Textarea 示例").style(title_style),
                Text::new("下面的内容读取自当前示例的 `main.rs`，你可以编辑它，但修改不会保存到磁盘。")
                    .user_select(true),
                Text::new(self.path_label.binding()).user_select(true),
                Flex::vertical()
                    .padding(Insets::all(dp(14.0)))
                    .gap(dp(10.0))
                    .width(pct(100.0))
                    .min_height(dp(0.0))
                    .grow(1.0)
                    .style(card_style)
                    .child(el![
                        Text::new("源码编辑区"),
                        Textarea::new(self.content.clone())
                            .width(pct(100.0))
                            .min_height(dp(0.0))
                            .grow(1.0)
                            .on_change(Command::new(|_app: &mut App| {
                                tgui_log(LogLevel::Info, "11")
                            }))
                    ]),
            ])
            .into()
    }
}

fn source_path() -> PathBuf {
    PathBuf::from("D:\\Project\\Rust\\libs\\tgui\\src\\runtime\\mod.rs")
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .title("tgui textarea example")
        .window_size(dp(960.0), dp(640.0))
        .with_view_model(App::new)
        .root_view(App::view)
        .run()
}
