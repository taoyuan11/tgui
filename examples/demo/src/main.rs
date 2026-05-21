#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;
use tgui::prelude::*;

fn text_style(mode: ResolvedThemeMode, size: Sp) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.typography.size = size;
    style
}

fn title_style(mode: ResolvedThemeMode) -> TextWidgetStyle {
    let mut style = text_style(mode, sp(28.0));
    style.typography.weight = FontWeight::Medium;
    style
}

fn status_style(mode: ResolvedThemeMode) -> TextWidgetStyle {
    text_style(mode, sp(14.0))
}

fn playback_status_text(state: AudioPlaybackState) -> String {
    tgui_log(LogLevel::Info, format!("播放状态: {:?}", state));
    match state {
        AudioPlaybackState::Idle => "等待".to_string(),
        AudioPlaybackState::Loading => "加载中".to_string(),
        AudioPlaybackState::Ready => "准备".to_string(),
        AudioPlaybackState::Playing => "播放中".to_string(),
        AudioPlaybackState::Paused => "暂停中".to_string(),
        AudioPlaybackState::Buffering => "缓冲中".to_string(),
        AudioPlaybackState::Ended => "播放结束".to_string(),
        AudioPlaybackState::Error(error) => format!("播放出错: {error}"),
    }
}

fn card_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.background_blur = dp(12.0).into();
    style.surface.border_color = Some(Color::rgb(48, 58, 76).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(14.0).into());
    style
}

fn accent_tooltip_style(mode: ResolvedThemeMode) -> TooltipStyle {
    let mut style = TooltipStyle::default_for(mode);
    style.background = Color::hexa(0x4F9CF9FF);
    style.foreground = Color::hexa(0x0B1220FF);
    style.radius = dp(8.0);
    style
}

fn image_style(mode: ResolvedThemeMode) -> ImageStyle {
    let mut style = ImageStyle::default_for(mode);
    style.surface.border_radius = Some(dp(12.0).into());
    style
}

fn canvas_style(mode: ResolvedThemeMode) -> CanvasStyle {
    let mut style = CanvasStyle::default_for(mode);
    style.surface.background = Some(Color::rgb(15, 23, 42).into());
    style.surface.border_color = Some(Color::rgb(51, 65, 85).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(14.0).into());
    style
}

fn shadow_showcase_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    let _ = mode;
    style.surface.background = Some(Color::hexa(0xFFFFFFFF).into());
    style.surface.border_radius = Some(dp(50.0).into());
    style.surface.shadow = Some(
        Shadow {
            offset_x: dp(0.0),
            offset_y: dp(7.0),
            blur: dp(30.0),
            spread: dp(0.0),
            color: Color::hex(0x64646F),
        }
        .into(),
    );
    style
}

struct VideoPlayer {
    video_controller: VideoController,
    source: TextController
}

impl VideoPlayer {
    fn new(context: &ViewModelContext) -> Self {
        let source = context.text_controller(String::from("D:\\CloudMusic\\MV\\郭顶 - 凄美地.mp4"));
        let controller = VideoController::new(context);

        controller.playback_state().map(|state| {
            match state {
                VideoPlaybackState::Error(err) => {
                    tgui_log(LogLevel::Error, format!("播放出错: {err}"));
                }
                _ => {}
            }
        });

        // controller.set_volume(0.0);
        Self {
            video_controller: controller,
            source
        }
    }

    fn play(&mut self) {
        self.video_controller.play();
    }

    fn pause(&mut self) {
        self.video_controller.pause();
    }

    fn change_source(&mut self, source: String) -> Result<(), TguiError> {
        let video_source = if source.starts_with("http") {
            VideoSource::Url {
                url: source.clone(),
                headers: vec![],
            }
        } else {
            VideoSource::File(PathBuf::from(source.clone()))
        };

        self.video_controller.load(video_source)?;
        Ok(())
    }

}

struct App {
    theme: State<ThemeMode>,
    switch: State<bool>,
    checkbox: State<bool>,
    radio: State<bool>,
    slider_value: State<f32>,
    contact_method: State<String>,
    select_action: State<Option<String>>,
    notification_status: State<String>,
    input_text: TextController,
    textarea_text: TextController,
    audio_controller: AudioController,
    video_player: VideoPlayer
}

impl ViewModel for App {
    fn new(context: &ViewModelContext) -> Self {
        let audio = AudioController::new(context);
        audio.set_volume(0.8);
        Self {
            theme: context.state(ThemeMode::System),
            switch: context.state(false),
            checkbox: context.state(false),
            radio: context.state(false),
            slider_value: context.state(80.0),
            contact_method: context.state(String::from("system")),
            select_action: context.state(None),
            notification_status: context.state(String::from("尚未发送通知")),
            input_text: context.text_controller("D:\\CloudMusic\\music\\James Blunt - You Are Beautiful.flac"),
            textarea_text: context.text_controller(
                "这是一个受控 Textarea。\n你可以在这里输入多行内容，示例不会保存修改。",
            ),
            audio_controller: audio,
            video_player: VideoPlayer::new(context)
        }
    }

    fn view(&self) -> Element<Self> {


        Flex::horizontal()
            .wrap(Wrap::Wrap)
            .padding(Insets::all(dp(20.0)))
            .gap(dp(10.0))
            .overflow_y(Overflow::Scroll)
            .child(el![
                Text::new("TGUI 组件列表示例")
                    .style(title_style)
                    .width(pct(100.0)),
                component_card(
                    "Text",
                    Text::new("这是一段可直接渲染、可复制的文本组件")
                        .user_select(true)
                        .style(|mode| text_style(mode, sp(16.0))),
                ),
                component_card(
                    "Button",
                    Flex::new(Axis::Horizontal).gap(dp(10.0)).child(el![
                        Button::new("普通按钮").primary(),
                        Button::new("次要按钮").secondary(),
                        Button::new("幽灵按钮").ghost(),
                        Button::new("危险按钮").danger(),
                        Button::new("禁用按钮").disable(true),
                    ]),
                ),
                component_card(
                    "Tooltip",
                    Flex::new(Axis::Vertical).gap(dp(10.0)).child(el![
                        Text::new("把鼠标悬停在按钮上看 Tooltip：四个方向 + 自动换行 + 自定义样式")
                            .style(status_style),
                        Flex::new(Axis::Horizontal).gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                            Button::new("上方")
                                .tooltip(Tooltip::new("默认 Placement::top()")),
                            Button::new("下方").tooltip(
                                Tooltip::new("浮层贴在按钮下边缘")
                                    .placement(OverlayPlacement::bottom()),
                            ),
                            Button::new("左侧").tooltip(
                                Tooltip::new("Placement::left()")
                                    .placement(OverlayPlacement::left()),
                            ),
                            Button::new("右侧").tooltip(
                                Tooltip::new("Placement::right()")
                                    .placement(OverlayPlacement::right()),
                            ),
                            Button::new("长文本").tooltip(Tooltip::new(
                                "Tooltip 会在超过 max_width 时自动换行，方便给较长的说明文字使用。",
                            )),
                            Button::new("强调样式").tooltip(
                                Tooltip::new("自定义 background / radius 即可换风格")
                                    .style(accent_tooltip_style(ResolvedThemeMode::Dark)),
                            ),
                        ]),
                    ]),
                ),
                component_card(
                    "Switch",
                    Switch::new(self.switch.signal()).on_change(ValueCommand::new(
                        |app: &mut App, enable| app.switch.set(enable)
                    )),
                ),
                component_card(
                    "Checkbox",
                    Checkbox::new(self.checkbox.signal())
                        .label("接收通知")
                        .on_change(ValueCommand::new(|app: &mut App, checked| {
                            app.checkbox.set(checked)
                        })),
                ),
                component_card(
                    "Radio",
                    Radio::new(self.radio.signal())
                        .label("单个单选框")
                        .on_change(ValueCommand::new(|app: &mut App, checked| {
                            app.radio.set(checked)
                        })),
                ),
                component_card(
                    "RadioGroup",
                    RadioGroup::new(
                        vec![
                            RadioOption::new("system".to_string(), "跟随系统".to_string()),
                            RadioOption::new("light".to_string(), "明亮".to_string()),
                            RadioOption::new("dark".to_string(), "暗淡".to_string()),
                        ],
                        self.contact_method.signal(),
                    )
                    .horizontal()
                    .on_change(ValueCommand::new(
                        |app: &mut App, (key, _label)| {
                            if key == "system" {
                                app.theme.set(ThemeMode::System)
                            } else if key == "light" {
                                app.theme.set(ThemeMode::Light)
                            } else {
                                app.theme.set(ThemeMode::Dark);
                            }
                            app.contact_method.set(key)
                        }
                    )),
                ),
                component_card(
                    "Select",
                    Select::new(
                        vec![
                            SelectOption::new("archive".to_string(), "归档".to_string()),
                            SelectOption::new("delete".to_string(), "删除".to_string())
                                .disable(true),
                            SelectOption::new("share".to_string(), "分享".to_string()),
                        ],
                        self.select_action.signal(),
                    )
                    .placeholder("请选择操作")
                    .width(dp(220.0))
                    .on_change(ValueCommand::new(
                        |app: &mut App, (key, _label)| { app.select_action.set(Some(key)) }
                    )),
                ),
                component_card(
                    "Shadow",
                    demo_shadow_card(),
                ),
                component_card(
                    "Slider",
                    Slider::new(self.slider_value.signal(), 0.0, 100.0)
                        .width(dp(240.0))
                        .step(5.0)
                        .show_ticks(true)
                        .show_value_label(true)
                        .format_value(|value| format!("{value:.0}%"))
                        .on_change(ValueCommand::new(|app: &mut App, value| {
                            app.slider_value.set(value);
                            app.audio_controller.set_volume(value / 100.0)
                        })),
                ),
                component_card(
                    "Audio",
                    self.build_audio_component()
                ),
                component_card(
                    "Video",
                    self.build_video_component()
                ),
                component_card(
                    "Input",
                    Input::new(self.input_text.clone())
                        .width(dp(260.0))
                        .placeholder("请在此输入需要播放的音乐绝对路径")
                ),
                component_card(
                    "Textarea",
                    Textarea::new(self.textarea_text.clone())
                        .size(dp(320.0), dp(140.0))
                        .placeholder("请输入多行内容")
                        .on_change(Command::new(|_app: &mut App| {})),
                ),
                component_card(
                    "Notification",
                    Flex::vertical().gap(dp(10.0)).child(el![
                        Flex::horizontal().gap(dp(10.0)).wrap(Wrap::Wrap).child(el![
                            Button::new("请求通知权限").on_click(Command::new_with_context(
                                |_: &mut App, ctx| { App::request_notification_permission(ctx) }
                            ),),
                            Button::new("发送普通通知").on_click(Command::new_with_context(
                                |app: &mut App, ctx| { app.send_plain_notification(ctx) }
                            ),),
                            Button::new("发送动作通知").on_click(Command::new_with_context(
                                |app: &mut App, ctx| { app.send_action_notification(ctx) }
                            ),),
                        ]),
                        Text::new(self.notification_status.signal()).style(status_style),
                    ]),
                ),
                component_card(
                    "Image",
                    Image::from_path(demo_image_path())
                        .size(dp(220.0), dp(120.0))
                        .style(image_style),
                ),
                component_card("Canvas", demo_canvas()),
            ])
            .into()
    }
}

fn component_card(title: &str, content: impl Into<Element<App>>) -> Element<App> {
    Flex::vertical()
        .gap(dp(10.0))
        .padding(Insets::all(dp(14.0)))
        .style(card_style)
        .child(el![
            Text::new(title).style(|mode| text_style(mode, sp(18.0))),
            content.into(),
        ])
        .into()
}

fn demo_image_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../background_effects/assets/juequling_shushu.jpg")
}

fn demo_canvas() -> Element<App> {
    Canvas::new(CanvasRecorder::build(|canvas| {
        canvas
            .set_fill(CanvasLinearGradient::new(
                Point::new(24.0, 20.0),
                Point::new(208.0, 128.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0x38BDF8FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                ],
            ))
            .set_stroke(CanvasStroke::new(dp(3.0), Color::hexa(0xE0F2FEFF)))
            .begin_path()
            .move_to(24.0, 20.0)
            .line_to(208.0, 20.0)
            .line_to(208.0, 128.0)
            .line_to(24.0, 128.0)
            .close_path()
            .fill_and_stroke();

        canvas
            .set_fill(Color::hexa(0x22C55EFF))
            .set_stroke(CanvasStroke::new(dp(3.0), Color::hexa(0x14532DFF)))
            .begin_path()
            .move_to(44.0, 146.0)
            .quad_to(116.0, 92.0, 188.0, 146.0)
            .line_to(188.0, 188.0)
            .line_to(44.0, 188.0)
            .close_path()
            .fill_and_stroke();
    }))
        .size(dp(232.0), dp(212.0))
        .style(canvas_style)
        .into()
}

fn demo_shadow_card() -> Element<App> {
    Flex::vertical()
        .gap(dp(12.0))
        .size(dp(200.0), dp(200.0))
        .center()
        .child(
            Stack::new()
                .size(dp(100.0), dp(100.0))
                .style(shadow_showcase_style)
        )
        .into()
}

impl App {

    fn build_video_component(&self) -> Element<Self> {
        Flex::vertical()
            .gap(dp(10.0))
            .child(el![
                Input::new(self.video_player.source.clone()).placeholder("在此输入视频地址"),
                VideoSurface::new(self.video_player.video_controller.clone()).size(dp(300.0), dp(168.0)),
                Flex::horizontal()
                .gap(dp(10.0))
                .child(el![
                    Button::new("加载")
                    .on_click(Command::new(|app: &mut App| {
                        if let Err(err) = app.video_player.change_source(app.video_player.source.text()) {
                            tgui_log(LogLevel::Error, format!("加载视频失败: {err}"))
                        }
                    })),
                    Button::new("播放")
                    .on_click(Command::new(|app: &mut App| {
                        app.video_player.play()
                    })),
                    Button::new("暂停")
                    .on_click(Command::new(|app: &mut App| {
                        app.video_player.pause()
                    }))
                ])
            ])
            .into()
    }

    fn build_audio_component(&self) -> Element<Self> {
        Flex::vertical()
            .gap(dp(10.0))
            .child(el![
                Audio::new(self.audio_controller.clone()),
                    Text::new(
                        self.audio_controller
                            .playback_state()
                            .map(playback_status_text),
                    ),
                    Flex::horizontal()
                    .gap(dp(10.0))
                    .child(el![
                        Button::new("加载")
                            .on_click(Command::new(|app: &mut App| {
                                match app.audio_controller.load(AudioSource::File(PathBuf::from(app.input_text.text()))) {
                                    Ok(()) => {}
                                    Err(err) => {
                                        tgui_log(LogLevel::Error, &err);
                                    }
                                }
                            })),
                        Button::new("播放")
                            .on_click(Command::new(|app: &mut App| {
                                app.audio_controller.play()
                            })),
                        Button::new("暂停")
                            .on_click(Command::new(|app: &mut App| {
                                app.audio_controller.pause()
                            }))
                    ])
                ]).into()
    }

    fn request_notification_permission(ctx: &CommandContext<Self>) {
        let _ =
            ctx.notifications()
                .request_permission(ValueCommand::new(|app: &mut App, result| {
                    app.notification_status.set(match result {
                        Ok(permission) => format!("通知权限: {permission:?}"),
                        Err(error) => format!("通知权限请求失败: {error}"),
                    });
                }));
    }

    fn send_plain_notification(&mut self, ctx: &CommandContext<Self>) {
        let result = ctx.notifications().send(
            NotificationOptions::new("TGUI Demo")
                .body("这是一条普通通知")
                .app_name("TGUI Demo"),
        );
        self.notification_status.set(match result {
            Ok(id) => format!("已发送普通通知: {id}"),
            Err(error) => {
                let string = format!("发送普通通知失败: {error}");
                tgui_log(LogLevel::Error, &string);
                string
            },
        });
    }

    fn send_action_notification(&mut self, ctx: &CommandContext<Self>) {
        let result = ctx.notifications().send_with_actions(
            NotificationOptions::new("TGUI Demo")
                .body("请选择一个动作，结果会回到 ViewModel。")
                .app_name("TGUI Demo")
                .action(NotificationAction::new("accept", "接受"))
                .action(NotificationAction::new("dismiss", "忽略")),
            ValueCommand::new(
                |app: &mut App, result: Result<NotificationActionEvent, NotificationError>| {
                    app.notification_status.set(match result {
                        Ok(event) => format!(
                            "通知动作: notification_id={}, action_id={}",
                            event.notification_id, event.action_id
                        ),
                        Err(error) => {
                            let string = format!("通知动作失败: {error}");
                            tgui_log(LogLevel::Error, &string);
                            string
                        },
                    });
                },
            ),
        );
        self.notification_status.set(match result {
            Ok(id) => format!("已发送动作通知: {id}"),
            Err(error) => {
                let string = format!("发送动作通知失败: {error}");
                tgui_log(LogLevel::Error, &string);
                string
            },
        });
    }

    fn theme_binding(&self) -> Signal<ThemeMode> {
        self.theme.signal()
    }

    fn run() -> Result<(), TguiError> {
        Application::new()
            .app_id("com.tgui.demo")
            .with_view_model(App::new)
            .root_view(App::view)
            .bind_theme_mode(App::theme_binding)
            .run()
    }
}

fn main() -> Result<(), TguiError> {
    App::run()
}
