#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;
use std::time::Duration;
use tgui::mvvm::ValidationErrors;
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

fn modern_toast_style(mode: ResolvedThemeMode) -> ToastStyle {
    let mut style = ToastStyle::default_for(mode);

    // 更现代的圆角
    style.radius = Value::Static(dp(10.0));

    // 增强阴影效果，营造更好的层次感
    style.shadow = Shadow {
        offset_x: dp(0.0),
        offset_y: dp(8.0),
        blur: dp(28.0),
        spread: dp(0.0),
        color: match mode {
            ResolvedThemeMode::Light => Color::rgba(0, 0, 0, 40),
            ResolvedThemeMode::Dark => Color::rgba(0, 0, 0, 120),
        },
    };

    // 调整边框
    style.border_width = Value::Static(dp(0.0));

    // 更舒适的内边距
    style.padding = Insets::all(dp(12.0));
    style.gap = dp(8.0);

    // 优化图标圆圈颜色 - 更鲜明、更现代的配色
    // Success - 清新的绿色
    style.success_icon_background = Value::Static(Color::hexa(0x10B981FF));
    style.success_icon_foreground = Value::Static(Color::hexa(0xFFFFFFFF));

    // Error - 醒目的红色
    style.error_icon_background = Value::Static(Color::hexa(0xEF4444FF));
    style.error_icon_foreground = Value::Static(Color::hexa(0xFFFFFFFF));

    // Warning - 温暖的橙色
    style.warning_icon_background = Value::Static(Color::hexa(0xF59E0BFF));
    style.warning_icon_foreground = Value::Static(Color::hexa(0xFFFFFFFF));

    // Info - 清澈的蓝色
    style.info_icon_background = Value::Static(Color::hexa(0x3B82F6FF));
    style.info_icon_foreground = Value::Static(Color::hexa(0xFFFFFFFF));

    // 调整文字样式
    style.title_text_style.weight = FontWeight::SemiBold;

    // 优化按钮样式
    style.action_button.min_height = dp(24.0);
    style.action_button.padding_x = dp(8.0);
    style.action_button.padding_y = dp(4.0);
    style.action_button.radius = Value::Static(dp(6.0));

    style.close_button.min_height = dp(20.0);
    style.close_button.padding_x = dp(4.0);
    style.close_button.padding_y = dp(3.0);

    // 调整最小和最大宽度
    style.min_width = dp(200.0);
    style.max_width = dp(320.0);

    // 增加 Toast 之间的间距
    style.stack_gap = dp(12.0);

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

        controller.set_volume(0.0);
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
    reduced_motion: State<bool>,
    switch: State<bool>,
    checkbox: State<bool>,
    radio: State<bool>,
    slider_value: State<f32>,
    contact_method: State<String>,
    select_action: State<Option<String>>,
    notification_status: State<String>,
    toast_status: State<String>,
    popover_open: State<bool>,
    popover_switch: State<bool>,
    popover_note: TextController,
    input_text: TextController,
    textarea_text: TextController,
    audio_controller: AudioController,
    video_player: VideoPlayer,
    toast_queue: ToastQueue<App>,
    toast_top_start: ToastQueue<App>,
    toast_top_center: ToastQueue<App>,
    toast_top_end: ToastQueue<App>,
    toast_bottom_start: ToastQueue<App>,
    toast_bottom_center: ToastQueue<App>,
    profile_form: Form,
    profile_name: TextFormField,
    profile_email: TextFormField,
    profile_newsletter: FormField<bool>,
    profile_status: State<String>,
    tabs_selected: State<String>,
    tabs_order: State<Vec<String>>,
    tabs_reorder_status: State<String>,
}

impl ViewModel for App {
    fn new(context: &ViewModelContext) -> Self {
        let audio = AudioController::new(context);
        audio.set_volume(0.8);
        let profile_form = Form::new(context);
        let profile_name = profile_form
            .text_field("name", "Alice Wonderland")
            .validator(|value| {
                if value.trim().is_empty() {
                    ValidationErrors::single("名称不能为空")
                } else {
                    ValidationErrors::none()
                }
            })
            .async_validator(|value| {
                if value.eq_ignore_ascii_case("admin") {
                    ValidationErrors::single("该名称已被保留")
                } else {
                    ValidationErrors::none()
                }
            });
        let profile_email = profile_form
            .text_field("email", "alice@example.com")
            .validator(|value| {
                if value.contains('@') {
                    ValidationErrors::none()
                } else {
                    ValidationErrors::single("请输入有效邮箱")
                }
            })
            .async_validator(|value| {
                if value.ends_with("@example.com") {
                    ValidationErrors::none()
                } else {
                    ValidationErrors::single("仅示例邮箱域名可通过异步校验")
                }
            });
        let profile_newsletter = profile_form.field("newsletter", true).validator(|enabled| {
            if *enabled {
                ValidationErrors::none()
            } else {
                ValidationErrors::single("建议至少订阅一项")
            }
        });
        Self {
            theme: context.state(ThemeMode::System),
            reduced_motion: context.state(false),
            switch: context.state(false),
            checkbox: context.state(false),
            radio: context.state(false),
            slider_value: context.state(80.0),
            contact_method: context.state(String::from("system")),
            select_action: context.state(None),
            notification_status: context.state(String::from("尚未发送通知")),
            toast_status: context.state(String::from("尚未触发 Toast 操作")),
            popover_open: context.state(false),
            popover_switch: context.state(true),
            popover_note: context.text_controller("预览状态下也可以直接编辑这里的内容。"),
            input_text: context.text_controller("D:\\CloudMusic\\music\\James Blunt - You Are Beautiful.flac"),
            textarea_text: context.text_controller(
                "这是一个受控 Textarea。\n你可以在这里输入多行内容，示例不会保存修改。",
            ),
            audio_controller: audio,
            video_player: VideoPlayer::new(context),
            toast_queue: ToastQueue::new(context),
            toast_top_start: ToastQueue::new(context),
            toast_top_center: ToastQueue::new(context),
            toast_top_end: ToastQueue::new(context),
            toast_bottom_start: ToastQueue::new(context),
            toast_bottom_center: ToastQueue::new(context),
            profile_form,
            profile_name,
            profile_email,
            profile_newsletter,
            profile_status: context.state("表单尚未提交".to_string()),
            tabs_selected: context.state("overview".to_string()),
            tabs_order: context.state(vec![
                "overview".to_string(),
                "settings".to_string(),
                "logs".to_string(),
                "metrics".to_string(),
                "advanced".to_string(),
            ]),
            tabs_reorder_status: context.state("尚未重排".to_string()),
        }
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .child(
                Flex::horizontal()
                    .size(pct(100.0), pct(100.0))
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
                        Text::new("把鼠标悬停或用 Tab 聚焦按钮查看 Tooltip：支持四个方向、自动换行、自定义样式、Esc 关闭与触摸长按。")
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
                            Button::new("富内容")
                                .tooltip(Tooltip::content(
                                    Flex::new(Axis::Vertical)
                                        .gap(dp(8.0))
                                        .padding(Insets::all(dp(10.0)))
                                        .child(Text::new("Tooltip 也能承载任意子树"))
                                        .child(Button::new("浮层里按钮").ghost()),
                                )),
                            Button::new("键盘聚焦")
                                .tooltip(Tooltip::new("按 Tab 聚焦后也会显示 Tooltip，按 Esc 可隐藏。")),
                        ]),
                        Text::new("提示：桌面端 hover 默认延迟约 500ms；键盘 focus 会立即显示；触摸端长按显示，松开后会短暂保留并在点击外部时关闭。")
                            .style(status_style),
                    ]),
                ),
                component_card("Popover", self.build_popover_component()),
                component_card("Form", self.build_form_component()),
                component_card("Menu", self.build_menu_component()),
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
                    "ProgressBar / Spinner",
                    self.build_progress_component(),
                ),
                component_card("Divider", self.build_divider_component()),
                component_card(
                    "Tabs / TabView",
                    self.build_tabs_component(),
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
                        Text::new(self.notification_status.signal()).style(status_style).user_select(true),
                    ]),
                ),
                component_card("Toast / Snackbar", self.build_toast_component()),
                component_card(
                    "Image",
                    Image::from_path(demo_image_path())
                        .size(dp(220.0), dp(120.0))
                        .style(image_style),
                ),
                component_card("Canvas", demo_canvas()),
            ])
            )
            .child(ToastHost::new(self.toast_queue.clone()).style(|mode| modern_toast_style(mode)))
            .child(ToastHost::new(self.toast_top_start.clone()).placement(ToastPlacement::TopStart).style(|mode| modern_toast_style(mode)))
            .child(ToastHost::new(self.toast_top_center.clone()).placement(ToastPlacement::TopCenter).style(|mode| modern_toast_style(mode)))
            .child(ToastHost::new(self.toast_top_end.clone()).placement(ToastPlacement::TopEnd).style(|mode| modern_toast_style(mode)))
            .child(ToastHost::new(self.toast_bottom_start.clone()).placement(ToastPlacement::BottomStart).style(|mode| modern_toast_style(mode)))
            .child(ToastHost::new(self.toast_bottom_center.clone()).placement(ToastPlacement::BottomCenter).style(|mode| modern_toast_style(mode)))
            .into()
    }
}
//
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
//
fn demo_image_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../background_effects/assets/juequling_shushu.jpg")
}
//
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
//
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
//
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

fn demo_tab_items(
    order: Vec<String>,
    slider_value: Signal<f32>,
    switch_value: Signal<bool>,
    checkbox_value: Signal<bool>,
    selected: Signal<String>,
) -> Vec<TabItem<App>> {
    order
        .into_iter()
        .map(|key| match key.as_str() {
            "overview" => TabItem::new(
                "overview",
                "概览",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Text::new("Tabs 会根据选中 key 只渲染当前 panel。").style(status_style),
                    ProgressBar::new(slider_value.clone().map(|value| value / 100.0))
                        .width(dp(240.0))
                        .show_label(true)
                        .label(slider_value.clone().map(|value| format!("当前音量 {:.0}%", value))),
                ]),
            ),
            "settings" => TabItem::new(
                "settings",
                "设置",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Text::new("这里放置可交互内容，切换 tab 不需要额外容器代码。")
                        .style(status_style),
                    Switch::new(switch_value.clone()).on_change(ValueCommand::new(
                        |app: &mut App, enabled| app.switch.set(enabled),
                    )),
                    Checkbox::new(checkbox_value.clone())
                        .label("同步到偏好设置")
                        .on_change(ValueCommand::new(|app: &mut App, checked| {
                            app.checkbox.set(checked)
                        })),
                ]),
            ),
            "logs" => TabItem::new(
                "logs",
                "日志",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Text::new("日志面板展示当前 tab 状态。").style(status_style),
                    Text::new(selected.clone().map(|key| format!("active tab: {key}")))
                        .style(status_style),
                ]),
            ),
            "metrics" => TabItem::new(
                "metrics",
                "指标",
                Text::new("More 模式下会被收进更多菜单。").style(status_style),
            ),
            _ => TabItem::new(
                "advanced",
                "高级",
                Text::new("支持拖拽重排的 tab。").style(status_style),
            ),
        })
        .collect()
}

impl App {
    fn build_tabs_component(&self) -> Element<Self> {
        let slider_value = self.slider_value.signal();
        let switch_value = self.switch.signal();
        let checkbox_value = self.checkbox.signal();
        let selected = self.tabs_selected.signal();
        let selected_for_items = selected.clone();
        let selected_for_tabs = selected.clone();
        Flex::vertical()
            .gap(dp(10.0))
            .child(self.tabs_order.signal().map(move |order| {
                let tabs: Element<App> = Tabs::new(
                    demo_tab_items(
                        order,
                        slider_value.clone(),
                        switch_value.clone(),
                        checkbox_value.clone(),
                        selected_for_items.clone(),
                    ),
                    selected_for_tabs.clone(),
                )
                .overflow_mode(TabsOverflowMode::More)
                .reorderable(true)
                .width(dp(360.0))
                .on_change(ValueCommand::new(|app: &mut App, (key, _label)| {
                    app.tabs_selected.set(key);
                }))
                .on_reorder(ValueCommand::new(|app: &mut App, event: TabsReorderEvent| {
                    let mut order = app.tabs_order.get();
                    if event.from_index < order.len() && event.to_index < order.len() {
                        let item = order.remove(event.from_index);
                        order.insert(event.to_index, item);
                        app.tabs_order.set(order);
                        app.tabs_selected.set(event.key.clone());
                        app.tabs_reorder_status.set(format!(
                            "重排 {} -> {}",
                            event.key, event.target_key
                        ));
                    }
                }))
                .into();
                tabs
            }))
            .child(Text::new(
                self.tabs_reorder_status
                    .signal()
                    .map(|value| format!("Tabs 状态：{value}")),
            ).style(status_style))
            .into()
    }

    fn build_form_component(&self) -> Element<Self> {
        let name_validation = self.profile_name.validation_state();
        let email_validation = self.profile_email.validation_state();
        let newsletter_validation = self.profile_newsletter.validation_state();
        Flex::vertical()
            .gap(dp(10.0))
            .child(el![
                Text::new("Form 统一管理值、校验和异步提交。字段错误态会同步到输入控件样式。")
                    .style(status_style),
                Input::new(self.profile_name.controller())
                    .placeholder("请输入名称")
                    .width(dp(260.0))
                    .validation(name_validation),
                Text::new(self.profile_name.first_error().map(|v| v.unwrap_or_default()))
                    .style(status_style),
                Input::new(self.profile_email.controller())
                    .placeholder("name@example.com")
                    .width(dp(260.0))
                    .validation(email_validation),
                Text::new(self.profile_email.first_error().map(|v| v.unwrap_or_default()))
                    .style(status_style),
                Checkbox::new(self.profile_newsletter.signal())
                    .label("订阅每周邮件")
                    .validation(newsletter_validation)
                    .on_change(self.profile_newsletter.bind_change()),
                Flex::horizontal().gap(dp(8.0)).child(el![
                    Button::new("验证").on_click(Command::new_with_context(|app: &mut App, ctx| {
                        let command = app.profile_form.validate_async_command::<App>();
                        command.execute_with_context(app, ctx);
                    })),
                    Button::new("提交").primary().on_click(Command::new_with_context(|app: &mut App, ctx| {
                        let form = app.profile_form.clone();
                        let command = form.submit_async_command(ValueCommand::new(|app: &mut App, snapshot: FormSnapshot| {
                            let name = snapshot.get::<String>("name").unwrap_or_default();
                            let email = snapshot.get::<String>("email").unwrap_or_default();
                            app.profile_status.set(format!("已提交: {name} / {email}"));
                        }));
                        command.execute_with_context(app, ctx);
                    })),
                    Button::new("重置").ghost().on_click(Command::new(|app: &mut App| {
                        app.profile_form.reset();
                        app.profile_status.set("表单已重置".to_string());
                    })),
                ]),
                Text::new(self.profile_form.status().map(|status| {
                    format!(
                        "validating={}, submitting={}",
                        status.validating,
                        status.submitting
                    )
                })).style(status_style),
                Text::new(self.profile_form.is_valid().map(|valid| {
                    if valid {
                        "表单当前无错误".to_string()
                    } else {
                        "表单当前存在错误".to_string()
                    }
                })).style(status_style),
                Text::new(self.profile_status.signal()).style(status_style),
            ])
            .into()
    }

    fn build_menu_component(&self) -> Element<Self> {
        let uncontrolled = MenuBar::uncontrolled()
            .entry(
                "文件",
                vec![
                    MenuItem::new("新建"),
                    MenuItem::separator(),
                    MenuItem::new("退出"),
                ],
            )
            .entry(
                "编辑",
                vec![
                    MenuItem::new("撤销"),
                    MenuItem::new("重做"),
                ],
            )
            .entry(
                "视图",
                vec![
                    MenuItem::new("缩放"),
                    MenuItem::new("全屏"),
                ],
            );

        Flex::vertical()
            .gap(dp(8.0))
            .child(Text::new("MenuBar.uncontrolled() 由 runtime 接管展开状态。").style(status_style))
            .child(uncontrolled)
            .into()
    }

    fn build_progress_component(&self) -> Element<Self> {
        Flex::vertical()
            .gap(dp(12.0))
            .child(el![
                Text::new("展示确定态 / 不确定态进度以及 reduced-motion 动画退化。")
                    .style(status_style),
                ProgressBar::new(self.slider_value.signal().map(|value| value / 100.0))
                    .width(dp(260.0))
                    .show_label(true)
                    .label(
                        self.slider_value
                            .signal()
                            .map(|value| format!("下载进度 {:.0}%", value)),
                    ),
                ProgressBar::indeterminate(true)
                    .width(dp(260.0))
                    .show_label(true)
                    .label("处理中"),
                Flex::horizontal().gap(dp(12.0)).wrap(Wrap::Wrap).child(el![
                    Spinner::new(),
                    Spinner::new().size(dp(32.0), dp(32.0)).style(|mode| {
                        let mut style = SpinnerStyle::default_for(mode);
                        style.indicator_color = Color::hexa(0xF97316FF).into();
                        style.track_color = Color::hexa(0xFDBA7488).into();
                        style.size = dp(32.0);
                        style.thickness = dp(4.0);
                        style
                    }),
                ]),
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Switch::new(self.reduced_motion.signal()).on_change(ValueCommand::new(
                        |app: &mut App, enabled| app.reduced_motion.set(enabled),
                    )),
                    Text::new(
                        self.reduced_motion.signal().map(|enabled| {
                            if enabled {
                                "reduced-motion: 已开启（停止循环动画）".to_string()
                            } else {
                                "reduced-motion: 已关闭（播放循环动画）".to_string()
                            }
                        }),
                    )
                    .style(status_style),
                ]),
            ])
            .into()
    }

    fn build_divider_component(&self) -> Element<Self> {
        Flex::vertical()
            .gap(dp(12.0))
            .child(el![
                Text::new("默认水平分隔线").style(status_style),
                Divider::new().width(dp(280.0)),
                Text::new("带标签的分隔线").style(status_style),
                Divider::new().width(dp(280.0)).label("或"),
                Text::new("虚线 + 自定义颜色 / 粗细").style(status_style),
                Divider::new()
                    .width(dp(280.0))
                    .dashed(true)
                    .thickness(dp(2.0))
                    .color(Color::hexa(0x2563EBFF)),
                Text::new("两端内缩").style(status_style),
                Divider::new().width(dp(280.0)).end_inset(dp(40.0)),
                Text::new("垂直分隔线").style(status_style),
                Flex::horizontal().gap(dp(12.0)).child(el![
                    Text::new("左侧").style(status_style),
                    Divider::new().vertical().height(dp(24.0)),
                    Text::new("中间").style(status_style),
                    Divider::new().vertical().height(dp(24.0)),
                    Text::new("右侧").style(status_style),
                ]),
            ])
            .into()
    }

    fn build_toast_component(&self) -> Element<Self> {
        Flex::vertical()
            .gap(dp(10.0))
            .child(el![
                Text::new("用于应用内短提示；现已优化为现代化设计，支持语义色、可选 action、持久提示以及桌面端 hover 暂停倒计时。")
                    .style(status_style),
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Button::new("✓ Success").on_click(Command::new(|app: &mut App| {
                        app.toast_queue.push(
                            Toast::new("文件已成功保存到云端")
                                .title("保存成功")
                                .kind(ToastKind::Success)
                        );
                        app.toast_status.set("最近操作：success toast".to_string());
                    })),
                    Button::new("✕ Error").danger().on_click(Command::new(|app: &mut App| {
                        app.toast_queue.push(
                            Toast::new("网络连接失败，请检查您的网络设置")
                                .title("上传失败")
                                .kind(ToastKind::Error)
                        );
                        app.toast_status.set("最近操作：error toast".to_string());
                    })),
                    Button::new("⚠ Warning").secondary().on_click(Command::new(|app: &mut App| {
                        app.toast_queue.push(
                            Toast::new("检测到网络波动，已自动切换为离线模式")
                                .title("网络提醒")
                                .kind(ToastKind::Warning)
                        );
                        app.toast_status.set("最近操作：warning toast".to_string());
                    })),
                    Button::new("ℹ Info").ghost().on_click(Command::new(|app: &mut App| {
                        app.toast_queue.push(
                            Toast::new("后台同步将在 30 秒后自动开始")
                                .title("提示")
                                .kind(ToastKind::Info)
                        );
                        app.toast_status.set("最近操作：info toast".to_string());
                    })),
                ]),
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Button::new("↶ 撤销示例").on_click(Command::new(|app: &mut App| {
                        app.toast_queue.push(
                            Toast::new("文件已移入回收站")
                                .title("Snackbar")
                                .kind(ToastKind::Info)
                                .action(ToastAction::new(
                                    "撤销",
                                    Command::new(|app: &mut App| {
                                        app.toast_status.set("最近操作：点击了撤销".to_string());
                                    }),
                                )),
                        );
                        app.toast_status.set("最近操作：弹出撤销 snackbar".to_string());
                    })),
                    Button::new("📌 持久提示").on_click(Command::new(|app: &mut App| {
                        app.toast_queue.push(
                            Toast::new("正在连接远程设备，完成后请手动关闭此提示")
                                .title("持久连接")
                                .kind(ToastKind::Warning)
                                .persistent(true)
                                .show_close_button(true),
                        );
                        app.toast_status.set("最近操作：弹出持久 toast".to_string());
                    })),
                    Button::new("⚡ 短时提示").secondary().on_click(Command::new(|app: &mut App| {
                        app.toast_queue.push(
                            Toast::new("这个提示将在 2 秒后自动消失")
                                .title("快速提示")
                                .kind(ToastKind::Success)
                                .duration(Duration::from_secs(2)),
                        );
                        app.toast_status.set("最近操作：弹出 2 秒 toast".to_string());
                    })),
                ]),
                Text::new("位置示例：").style(status_style),
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Button::new("↖ 左上").ghost().on_click(Command::new(|app: &mut App| {
                        app.toast_top_start.push(
                            Toast::new("左上角弹出的提示")
                                .title("TopStart")
                                .kind(ToastKind::Info)
                                .duration(Duration::from_secs(3)),
                        );
                        app.toast_status.set("最近操作：左上 toast".to_string());
                    })),
                    Button::new("↑ 顶部居中").ghost().on_click(Command::new(|app: &mut App| {
                        app.toast_top_center.push(
                            Toast::new("顶部居中弹出的提示")
                                .title("TopCenter")
                                .kind(ToastKind::Success)
                                .duration(Duration::from_secs(3)),
                        );
                        app.toast_status.set("最近操作：顶部居中 toast".to_string());
                    })),
                    Button::new("↗ 右上").ghost().on_click(Command::new(|app: &mut App| {
                        app.toast_top_end.push(
                            Toast::new("右上角弹出的提示")
                                .title("TopEnd")
                                .kind(ToastKind::Warning)
                                .duration(Duration::from_secs(3)),
                        );
                        app.toast_status.set("最近操作：右上 toast".to_string());
                    })),
                    Button::new("↙ 左下").ghost().on_click(Command::new(|app: &mut App| {
                        app.toast_bottom_start.push(
                            Toast::new("左下角弹出的提示")
                                .title("BottomStart")
                                .kind(ToastKind::Error)
                                .duration(Duration::from_secs(3)),
                        );
                        app.toast_status.set("最近操作：左下 toast".to_string());
                    })),
                    Button::new("↓ 底部居中").ghost().on_click(Command::new(|app: &mut App| {
                        app.toast_bottom_center.push(
                            Toast::new("底部居中弹出的提示")
                                .title("BottomCenter")
                                .kind(ToastKind::Success)
                                .duration(Duration::from_secs(3)),
                        );
                        app.toast_status.set("最近操作：底部居中 toast".to_string());
                    })),
                    Button::new("↘ 右下 (默认)").ghost().on_click(Command::new(|app: &mut App| {
                        app.toast_queue.push(
                            Toast::new("右下角弹出的提示（默认位置）")
                                .title("BottomEnd / Adaptive")
                                .kind(ToastKind::Info)
                                .duration(Duration::from_secs(3)),
                        );
                        app.toast_status.set("最近操作：右下 toast（默认）".to_string());
                    })),
                ]),
                Text::new(self.toast_status.signal()).style(status_style),
            ])
            .into()
    }
    fn build_popover_component(&self) -> Element<Self> {
        let trigger_text = self
            .popover_open
            .signal()
            .map(|open| if open { "已固定打开" } else { "点击或悬停打开" }.to_string());

        Flex::vertical()
            .gap(dp(10.0))
            .child(el![
                Text::new("支持点击固定打开，也支持 hover 预览；鼠标移入浮层后可以继续交互，点击外部或按 Esc 关闭固定打开态。")
                    .style(status_style),
                Popover::new(
                    Button::new(trigger_text)
                        .secondary()
                        .size(dp(180.0), dp(36.0)),
                )
                .content(
                    Flex::vertical()
                        .gap(dp(12.0))
                        .width(dp(280.0))
                        .padding(Insets::all(dp(16.0)))
                        .style(|mode| {
                            let mut style = ContainerStyle::default_for(mode);
                            // 根据主题模式设置背景色
                            style.surface.background = Some(match mode {
                                ResolvedThemeMode::Light => Color::hexa(0xFFFFFFFF),
                                ResolvedThemeMode::Dark => Color::hexa(0x1E293BFF),
                            }.into());
                            // 添加圆角
                            style.surface.border_radius = Some(dp(12.0).into());
                            // 添加边框
                            style.surface.border_width = Some(dp(1.0).into());
                            style.surface.border_color = Some(match mode {
                                ResolvedThemeMode::Light => Color::hexa(0xE2E8F0FF),
                                ResolvedThemeMode::Dark => Color::hexa(0x334155FF),
                            }.into());
                            // 添加阴影，营造浮起效果
                            style.surface.shadow = Some(Shadow {
                                offset_x: dp(0.0),
                                offset_y: dp(8.0),
                                blur: dp(24.0),
                                spread: dp(0.0),
                                color: match mode {
                                    ResolvedThemeMode::Light => Color::rgba(0, 0, 0, 31),
                                    ResolvedThemeMode::Dark => Color::rgba(0, 0, 0, 102),
                                },
                            }.into());
                            // 添加背景模糊效果
                            style.surface.background_blur = dp(16.0).into();
                            style
                        })
                        .child(el![
                            Text::new("快速设置").style(|mode| {
                                let mut style = text_style(mode, sp(18.0));
                                style.typography.weight = FontWeight::SemiBold;
                                style
                            }),
                            Text::new("同一个 Popover 同时展示 Click 固定打开和 Hover 预览。")
                                .style(status_style),
                            Input::new(self.popover_note.clone())
                                .placeholder("输入浮层里的备注")
                                .width(dp(248.0)),
                            Switch::new(self.popover_switch.signal()).on_change(ValueCommand::new(
                                |app: &mut App, enabled| app.popover_switch.set(enabled)
                            )),
                            Checkbox::new(self.checkbox.signal())
                                .label("沿用页面里的 checkbox 状态")
                                .on_change(ValueCommand::new(|app: &mut App, checked| {
                                    app.checkbox.set(checked)
                                })),
                            Flex::horizontal().gap(dp(8.0)).child(el![
                                Button::new("应用")
                                    .primary()
                                    .on_click(Command::new(|app: &mut App| {
                                        app.popover_open.set(false)
                                    })),
                                Button::new("关闭")
                                    .ghost()
                                    .on_click(Command::new(|app: &mut App| {
                                        app.popover_open.set(false)
                                    })),
                            ]),
                        ]),
                )
                .open(self.popover_open.signal())
                .on_open_change(ValueCommand::new(|app: &mut App, open| {
                    app.popover_open.set(open)
                }))
                .style({
                    let mut style = PopoverStyle::default_for(ResolvedThemeMode::Light);
                    style.pointer_size = Some(dp(10.0));
                    style
                })
                .trigger_mode(PopoverTriggerMode::ClickAndHoverPreview),
                Text::new(
                    self.popover_switch
                        .signal()
                        .map(|enabled| if enabled { "浮层内开关：开启" } else { "浮层内开关：关闭" }.to_string()),
                )
                .style(status_style),
            ])
            .into()
    }
    //
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
    //
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
    //
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
    //
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
    //
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
    //
    fn theme_binding(&self) -> Signal<ThemeMode> {
        self.theme.signal()
    }

    fn reduced_motion_binding(&self) -> Signal<bool> {
        self.reduced_motion.signal()
    }

    fn run() -> Result<(), TguiError> {
        Application::new()
            .app_id("com.tgui.demo")
            .with_view_model(App::new)
            .root_view(App::view)
            .bind_theme_mode(App::theme_binding)
            .bind_reduced_motion(App::reduced_motion_binding)
            .run()
    }
}

fn main() -> Result<(), TguiError> {
    App::run()
}
