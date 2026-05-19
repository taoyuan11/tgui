use std::path::PathBuf;
use std::time::Duration;

#[cfg(target_os = "android")]
use tgui::platform::android::activity::AndroidApp;
use tgui::prelude::*;

// ---------- 样式 ----------

fn text_style(mode: ResolvedThemeMode, size: Sp) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.typography.size = size;
    style
}

fn title_style(mode: ResolvedThemeMode) -> TextWidgetStyle {
    let mut style = text_style(mode, sp(26.0));
    style.typography.weight = FontWeight::Medium;
    style
}

fn section_title_style(mode: ResolvedThemeMode) -> TextWidgetStyle {
    let mut style = text_style(mode, sp(18.0));
    style.typography.weight = FontWeight::Medium;
    style
}

fn body_style(mode: ResolvedThemeMode) -> TextWidgetStyle {
    text_style(mode, sp(14.0))
}

fn card_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.border_radius = Some(dp(16.0).into());
    style.surface.border_color = Some(Color::hexa(0x2A4060FF).into());
    style.surface.border_width = Some(dp(1.0).into());
    style
}

fn canvas_style(mode: ResolvedThemeMode) -> CanvasStyle {
    let mut style = CanvasStyle::default_for(mode);
    style.surface.background = Some(Color::hexa(0x0F1B2DFF).into());
    style.surface.border_radius = Some(dp(12.0).into());
    style
}

// ---------- ViewModel ----------

struct AndroidShowcaseVm {
    // 主题
    theme: State<ThemeMode>,
    theme_choice: State<String>,

    // 表单控件
    switch_on: State<bool>,
    checkbox_on: State<bool>,
    radio_choice: State<String>,
    select_value: State<Option<String>>,

    // 滑块
    slider_value: State<f32>,

    // 输入框
    input_text: TextController,
    textarea_text: TextController,

    // 对话框状态回显
    message_status: State<String>,
    file_status: State<String>,

    // 动画演示
    expanded: State<bool>,

    // 计数器
    counter: State<u32>,
}

impl AndroidShowcaseVm {
    fn theme_signal(&self) -> Signal<ThemeMode> {
        self.theme.signal()
    }

    fn pick_theme(&mut self, key: String) {
        let mode = match key.as_str() {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::System,
        };
        self.theme.set(mode);
        self.theme_choice.set(key);
    }

    fn increment(&mut self) {
        self.counter.update(|value| *value += 1);
    }

    fn show_info_dialog(ctx: &CommandContext<Self>) {
        let _ = ctx.dialogs().show_message_async(
            MessageDialogOptions::new()
                .title("信息")
                .description("这是来自 tgui 的 Android AlertDialog。")
                .level(MessageDialogLevel::Info)
                .buttons(MessageDialogButtons::Ok),
            ValueCommand::new(Self::apply_message_result),
        );
    }

    fn show_confirm_dialog(ctx: &CommandContext<Self>) {
        let _ = ctx.dialogs().show_message_async(
            MessageDialogOptions::new()
                .title("确认")
                .description("你想要继续操作吗？")
                .level(MessageDialogLevel::Warning)
                .buttons(MessageDialogButtons::YesNoCancel),
            ValueCommand::new(Self::apply_message_result),
        );
    }

    fn pick_file(ctx: &CommandContext<Self>) {
        let _ = ctx.dialogs().open_file_async(
            FileDialogOptions::new()
                .title("选择文件")
                .add_filter("图片", &["png", "jpg", "jpeg", "webp"]),
            ValueCommand::new(Self::apply_file_result),
        );
    }

    fn pick_folder(ctx: &CommandContext<Self>) {
        let _ = ctx.dialogs().pick_folder_async(
            FileDialogOptions::new().title("选择目录"),
            ValueCommand::new(Self::apply_file_result),
        );
    }

    fn apply_message_result(&mut self, result: Result<MessageDialogResult, DialogError>) {
        let text = match result {
            Ok(choice) => format!("按钮: {choice:?}"),
            Err(error) => format!("失败: {error}"),
        };
        self.message_status.set(text);
    }

    fn apply_file_result(&mut self, result: Result<Option<PathBuf>, DialogError>) {
        let text = match result {
            Ok(Some(path)) => format!("已选择: {}", path.display()),
            Ok(None) => "已取消".to_string(),
            Err(error) => format!("失败: {error}"),
        };
        self.file_status.set(text);
    }

    fn toggle_animation(&mut self) {
        self.expanded.update(|value| *value = !*value);
    }

    fn animated_card_width(&self) -> Signal<Dp> {
        self.expanded
            .signal()
            .map(|expanded| if expanded { dp(280.0) } else { dp(160.0) })
            .animated(Transition::ease_in_out(Duration::from_millis(320)))
    }

    fn animated_card_background(&self) -> Signal<Color> {
        self.expanded
            .signal()
            .map(|expanded| {
                if expanded {
                    Color::hexa(0x0F766EFF)
                } else {
                    Color::hexa(0x9333EAFF)
                }
            })
            .animated(Transition::ease_in_out(Duration::from_millis(320)))
    }

    fn animated_card_radius(&self) -> Signal<Dp> {
        self.expanded
            .signal()
            .map(|expanded| if expanded { dp(28.0) } else { dp(12.0) })
            .animated(Transition::ease_out(Duration::from_millis(260)))
    }

    fn animated_card_label(&self) -> Signal<String> {
        self.expanded.signal().map(|expanded| {
            if expanded {
                "已展开 (Collapse)".to_string()
            } else {
                "已折叠 (Expand)".to_string()
            }
        })
    }
}

impl ViewModel for AndroidShowcaseVm {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            theme: context.state(ThemeMode::Dark),
            theme_choice: context.state("dark".to_string()),
            switch_on: context.state(true),
            checkbox_on: context.state(false),
            radio_choice: context.state("medium".to_string()),
            select_value: context.state(Some("share".to_string())),
            slider_value: context.state(40.0),
            input_text: context.text_controller("hello tgui"),
            textarea_text: context.text_controller(
                "你可以在这里输入多行文本。\nTgui 的 Input / Textarea 支持 IME、选择、滚动。",
            ),
            message_status: context.state("尚未触发消息对话框".to_string()),
            file_status: context.state("尚未选择文件 / 目录".to_string()),
            expanded: context.state(false),
            counter: context.state(0),
        }
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .child(
                Flex::vertical()
                    .size(pct(100.0), pct(100.0))
                    .padding(Insets::all(dp(16.0)))
                    .gap(dp(14.0))
                    .overflow_y(Overflow::Scroll)
                    .child(el![
                        Text::new("tgui Android Showcase").style(title_style),
                        Text::new("展示 Android 平台上可用的所有 tgui 组件与能力").style(body_style),
                        self.theme_section(),
                        self.text_section(),
                        self.button_section(),
                        self.toggle_section(),
                        self.choice_section(),
                        self.slider_section(),
                        self.input_section(),
                        self.canvas_section(),
                        self.dialog_section(),
                        self.animation_section(),
                    ]),
            )
            .into()
    }
}

// ---------- 分节 ----------

fn header_section() -> Element<AndroidShowcaseVm> {
    Flex::vertical()
        .gap(dp(4.0))
        .width(pct(100.0))
        .child(el![
            Text::new("tgui Android Showcase").style(title_style),
            Text::new("展示 Android 平台上可用的所有 tgui 组件与能力").style(body_style),
        ])
        .into()
}

fn section<Body: Into<Element<AndroidShowcaseVm>>>(
    title: &str,
    body: Body,
) -> Element<AndroidShowcaseVm> {
    Flex::vertical()
        .width(pct(100.0))
        .gap(dp(10.0))
        .padding(Insets::all(dp(14.0)))
        .style(card_style)
        .child(el![
            Text::new(title.to_string()).style(section_title_style),
            body.into(),
        ])
        .into()
}

impl AndroidShowcaseVm {
    fn theme_section(&self) -> Element<Self> {
        section(
            "主题切换",
            RadioGroup::new(
                vec![
                    RadioOption::new("system".to_string(), "跟随系统".to_string()),
                    RadioOption::new("light".to_string(), "明亮".to_string()),
                    RadioOption::new("dark".to_string(), "暗黑".to_string()),
                ],
                self.theme_choice.signal(),
            )
            .horizontal()
            .on_change(ValueCommand::new(|vm: &mut Self, (key, _label)| {
                vm.pick_theme(key)
            })),
        )
    }

    fn text_section(&self) -> Element<Self> {
        section(
            "Text",
            Flex::vertical().gap(dp(4.0)).child(el![
                Text::new("可选择文本（长按尝试）")
                    .user_select(true)
                    .style(|mode| text_style(mode, sp(16.0))),
                Text::new(
                    "Signal 驱动: 计数 = "
                        .to_string()
                )
                .style(body_style),
                Text::new(self.counter.signal().map(|n| format!("{n}"))).style(body_style),
            ]),
        )
    }

    fn button_section(&self) -> Element<Self> {
        section(
            "Button",
            Flex::vertical().gap(dp(8.0)).child(el![
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Button::new("Primary").primary(),
                    Button::new("Secondary").secondary(),
                    Button::new("Ghost").ghost(),
                    Button::new("Danger").danger(),
                    Button::new("Disabled").disable(true),
                ]),
                Button::new(
                    self.counter
                        .signal()
                        .map(|n| format!("+1 计数器 ({n})"))
                )
                .primary()
                .on_click(Command::new(Self::increment)),
            ]),
        )
    }

    fn toggle_section(&self) -> Element<Self> {
        section(
            "Switch / Checkbox",
            Flex::horizontal().gap(dp(16.0)).wrap(Wrap::Wrap).child(el![
                Switch::new(self.switch_on.signal()).on_change(ValueCommand::new(
                    |vm: &mut Self, on| vm.switch_on.set(on)
                )),
                Checkbox::new(self.checkbox_on.signal())
                    .label("订阅推送")
                    .on_change(ValueCommand::new(|vm: &mut Self, on| vm.checkbox_on.set(on))),
            ]),
        )
    }

    fn choice_section(&self) -> Element<Self> {
        section(
            "Radio / Select",
            Flex::vertical().gap(dp(10.0)).child(el![
                RadioGroup::new(
                    vec![
                        RadioOption::new("small".to_string(), "小".to_string()),
                        RadioOption::new("medium".to_string(), "中".to_string()),
                        RadioOption::new("large".to_string(), "大".to_string()),
                    ],
                    self.radio_choice.signal(),
                )
                .horizontal()
                .on_change(ValueCommand::new(|vm: &mut Self, (key, _)| vm
                    .radio_choice
                    .set(key))),
                Select::new(
                    vec![
                        SelectOption::new("archive".to_string(), "归档".to_string()),
                        SelectOption::new("share".to_string(), "分享".to_string()),
                        SelectOption::new("delete".to_string(), "删除".to_string())
                            .disable(true),
                    ],
                    self.select_value.signal(),
                )
                .placeholder("请选择操作")
                .width(pct(100.0))
                .on_change(ValueCommand::new(|vm: &mut Self, (key, _)| vm
                    .select_value
                    .set(Some(key)))),
            ]),
        )
    }

    fn slider_section(&self) -> Element<Self> {
        section(
            "Slider",
            Flex::vertical().gap(dp(8.0)).child(el![
                Slider::new(self.slider_value.signal(), 0.0, 100.0)
                    .width(pct(100.0))
                    .step(1.0)
                    .show_value_label(true)
                    .format_value(|value| format!("{value:.0}%"))
                    .on_change(ValueCommand::new(|vm: &mut Self, v| vm.slider_value.set(v))),
                Text::new(
                    self.slider_value
                        .signal()
                        .map(|v| format!("当前值: {v:.1}"))
                )
                .style(body_style),
            ]),
        )
    }

    fn input_section(&self) -> Element<Self> {
        section(
            "Input / Textarea",
            Flex::vertical().gap(dp(10.0)).child(el![
                Input::new(self.input_text.clone())
                    .width(pct(100.0))
                    .placeholder("单行输入框"),
                Textarea::new(self.textarea_text.clone())
                    .size(pct(100.0), dp(120.0))
                    .placeholder("多行输入框"),
            ]),
        )
    }

    fn canvas_section(&self) -> Element<Self> {
        let drawing = Canvas::new(CanvasRecorder::build(|canvas| {
            // 蓝绿渐变圆角矩形
            canvas
                .set_fill(CanvasLinearGradient::new(
                    Point::new(10.0, 10.0),
                    Point::new(280.0, 120.0),
                    vec![
                        CanvasGradientStop::new(0.0, Color::hexa(0x38BDF8FF)),
                        CanvasGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                    ],
                ))
                .set_stroke(CanvasStroke::new(dp(2.0), Color::hexa(0xE0F2FEFF)))
                .begin_path()
                .move_to(20.0, 16.0)
                .line_to(260.0, 16.0)
                .line_to(260.0, 120.0)
                .line_to(20.0, 120.0)
                .close_path()
                .fill_and_stroke();

            // 弧形绿色叶子
            canvas
                .set_fill(Color::hexa(0x22C55EFF))
                .set_stroke(CanvasStroke::new(dp(2.0), Color::hexa(0x14532DFF)))
                .begin_path()
                .move_to(40.0, 150.0)
                .quad_to(140.0, 96.0, 240.0, 150.0)
                .line_to(240.0, 196.0)
                .line_to(40.0, 196.0)
                .close_path()
                .fill_and_stroke();
        }))
        .size(pct(100.0), dp(220.0))
        .style(canvas_style);
        section("Canvas", drawing)
    }

    fn dialog_section(&self) -> Element<Self> {
        section(
            "Dialog (Android JNI)",
            Flex::vertical().gap(dp(8.0)).child(el![
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Button::new("信息对话框")
                        .primary()
                        .on_click(Command::new_with_context(
                            |_: &mut Self, ctx| Self::show_info_dialog(ctx)
                        )),
                    Button::new("YesNoCancel").on_click(Command::new_with_context(
                        |_: &mut Self, ctx| Self::show_confirm_dialog(ctx)
                    )),
                ]),
                Text::new(self.message_status.signal()).style(body_style),
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Button::new("选择文件").on_click(Command::new_with_context(
                        |_: &mut Self, ctx| Self::pick_file(ctx)
                    )),
                    Button::new("选择目录").on_click(Command::new_with_context(
                        |_: &mut Self, ctx| Self::pick_folder(ctx)
                    )),
                ]),
                Text::new(self.file_status.signal()).style(body_style),
            ]),
        )
    }

    fn animation_section(&self) -> Element<Self> {
        let background = self.animated_card_background();
        let radius = self.animated_card_radius();
        section(
            "Animation",
            Flex::vertical().gap(dp(10.0)).child(el![
                Button::new(self.animated_card_label())
                    .primary()
                    .on_click(Command::new(Self::toggle_animation)),
                Stack::new()
                    .size(pct(100.0), dp(160.0))
                    .center()
                    .child(
                        Flex::vertical()
                            .center()
                            .padding(Insets::all(dp(20.0)))
                            .gap(dp(4.0))
                            .width(self.animated_card_width())
                            .style(move |mode| {
                                let mut style = ContainerStyle::default_for(mode);
                                style.surface.background = Some(background.clone().into());
                                style.surface.border_radius = Some(radius.clone().into());
                                style
                            })
                            .child(el![
                                Text::new("Transition Demo")
                                    .style(|mode| text_style(mode, sp(16.0))),
                                Text::new("点击按钮以播放动画").style(body_style),
                            ])
                    ),
            ]),
        )
    }
}

// ---------- 入口 ----------

fn themed_app() -> Application {
    let mut dark = Theme::dark();
    dark.colors.background = Color::hexa(0x09111EFF);
    dark.colors.surface = Color::hexa(0x132238FF);
    dark.colors.surface_low = Color::hexa(0x1C3150FF);
    dark.colors.primary = Color::hexa(0x54A6FFFF);

    let light = Theme::light();
    let theme_set = ThemeSet::new(light, dark);

    Application::new()
        .app_id("com.tgui.android_basic_window")
        .title("tgui android showcase")
        .theme_set(theme_set)
        .theme_mode(ThemeMode::Dark)
}

#[cfg(target_os = "android")]
fn run_android_entry(app: AndroidApp) -> Result<(), TguiError> {
    themed_app()
        .with_view_model(AndroidShowcaseVm::new)
        .root_view(AndroidShowcaseVm::view)
        .bind_theme_mode(AndroidShowcaseVm::theme_signal)
        .run_android(app)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: AndroidApp) {
    if let Err(error) = run_android_entry(app) {
        panic!("failed to run android_basic_window: {error}");
    }
}
