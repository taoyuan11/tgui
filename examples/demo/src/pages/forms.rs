use crate::app::App;
use crate::demo_section::{self, UsageDemo};
use crate::styles;
use tgui::prelude::*;

const CODE_INPUT_BASIC: &str = r#"Input::new(app.input_text.clone())
    .width(dp(320.0))
    .placeholder("输入本地路径或 URL")"#;

const CODE_INPUT_VALIDATION: &str = r#"Input::new(app.profile_email.controller())
    .placeholder("name@example.com")
    .validation(app.profile_email.validation_state())"#;

const CODE_TEXTAREA_BASIC: &str = r#"Textarea::new(app.textarea_text.clone())
    .size(dp(340.0), dp(140.0))
    .placeholder("请输入多行内容")"#;

const CODE_TEXTAREA_STATUS: &str = r#"Textarea::new(app.textarea_text.clone())
    .on_change(Command::new(|app: &mut App| {
        app.profile_status.set("Textarea 已更新".to_string());
    }))"#;

const CODE_SWITCH_BASIC: &str = r#"Switch::new(app.switch.signal())
    .on_change(ValueCommand::new(|app: &mut App, enabled| {
        app.switch.set(enabled);
    }))"#;

const CODE_SWITCH_DISABLED: &str = r#"Flex::horizontal().gap(dp(10.0)).child(el![
    Switch::new(true),
    Switch::new(false).disable(true),
])"#;

const CODE_CHECKBOX_BASIC: &str = r#"Checkbox::new(app.checkbox.signal())
    .label("接收通知")
    .on_change(ValueCommand::new(|app: &mut App, checked| {
        app.checkbox.set(checked);
    }))"#;

const CODE_CHECKBOX_VALIDATION: &str = r#"Checkbox::new(app.profile_newsletter.signal())
    .label("订阅每周邮件")
    .validation(app.profile_newsletter.validation_state())
    .on_change(app.profile_newsletter.bind_change())"#;

const CODE_RADIO_BASIC: &str = r#"Radio::new(app.radio.signal())
    .label("单个单选框")
    .on_change(ValueCommand::new(|app: &mut App, checked| {
        app.radio.set(checked);
    }))"#;

const CODE_RADIO_GROUP: &str = r#"RadioGroup::new(options, app.contact_method.signal())
    .horizontal()
    .on_change(ValueCommand::new(|app: &mut App, (key, _label)| {
        app.contact_method.set(key);
    }))"#;

const CODE_SELECT_BASIC: &str = r#"Select::new(options, app.select_action.signal())
    .placeholder("请选择操作")
    .on_change(ValueCommand::new(|app: &mut App, (key, _label)| {
        app.select_action.set(Some(key));
    }))"#;

const CODE_SELECT_DISABLED: &str = r#"SelectOption::new("delete".to_string(), "删除".to_string())
    .disable(true)"#;

const CODE_COMBOBOX_BASIC: &str = r#"Combobox::new(app.combobox_text.clone(), options)
    .open(app.combobox_open.signal())
    .selected_key(app.combobox_selected.signal())
    .on_change(ValueCommand::new(|app: &mut App, change| {
        app.combobox_selected.set(change.selected_key);
    }))"#;

const CODE_SLIDER_BASIC: &str = r#"Slider::new(app.slider_value.signal(), 0.0, 100.0)
    .step(5.0)
    .show_value_label(true)
    .format_value(|value| format!("{value:.0}%"))"#;

const CODE_SLIDER_CONTROLLED: &str = r#"Slider::new(app.slider_value.signal(), 0.0, 100.0)
    .on_change(ValueCommand::new(|app: &mut App, value| {
        app.slider_value.set(value);
        app.audio_controller.set_volume(value / 100.0);
    }))"#;

const CODE_SLIDER_VERTICAL: &str = r#"Slider::new(app.slider_value.signal(), 0.0, 100.0)
    .vertical()
    .height(dp(180.0))
    .step(5.0)
    .show_ticks(true)
    .show_value_label(true)
    .format_value(|value| format!("{value:.0}%"))
    .on_change(ValueCommand::new(|app: &mut App, value| {
        app.slider_value.set(value);
    }))"#;

const CODE_RATING_BASIC: &str = r#"Rating::new(app.rating_value.signal())
    .half()
    .on_change(ValueCommand::new(|app: &mut App, change| {
        app.rating_value.set(change.value);
    }))"#;

const CODE_DATE_PICKER: &str = r#"DatePicker::new(app.demo_date_text.clone(), app.demo_date.signal(), app.demo_date_month.signal())
    .open(app.demo_date_open.signal())
    .on_open_change(ValueCommand::new(App::set_demo_date_open))
    .on_month_change(ValueCommand::new(App::set_demo_date_month))
    .on_change(ValueCommand::new(App::set_demo_date))"#;

const CODE_TIME_PICKER: &str = r#"TimePicker::new(app.demo_time_text.clone(), app.demo_time.signal())
    .open(app.demo_time_open.signal())
    .on_open_change(ValueCommand::new(App::set_demo_time_open))
    .on_change(ValueCommand::new(App::set_demo_time))"#;

const CODE_NUMBER_INPUT: &str = r#"NumberInput::new(app.demo_number_text.clone(), app.demo_number.signal())
    .range(0.0, 99.0)
    .step(1.0)
    .on_change(ValueCommand::new(App::set_demo_number))"#;

const CODE_COLOR_PICKER: &str = r#"ColorPicker::new(app.demo_color.signal())
    .open(app.demo_color_open.signal())
    .on_open_change(ValueCommand::new(App::set_demo_color_open))
    .on_change(ValueCommand::new(App::set_demo_color))"#;

const CODE_UPLOAD: &str = r#"Upload::new(app.upload_files.signal())
    .accept_extensions(&["png", "jpg", "pdf", "txt"])
    .max_files(8)
    .on_select(ValueCommand::new(App::add_upload_files))
    .on_remove(ValueCommand::new(App::remove_upload_file))"#;

const CODE_FORM_FIELDS: &str = r#"Input::new(app.profile_name.controller())
    .validation(app.profile_name.validation_state())

Checkbox::new(app.profile_newsletter.signal())
    .validation(app.profile_newsletter.validation_state())
    .on_change(app.profile_newsletter.bind_change())"#;

const CODE_FORM_SUBMIT: &str = r#"let command = app.profile_form.submit_async_command(
    ValueCommand::new(|app: &mut App, snapshot: FormSnapshot| {
        let name = snapshot.get::<String>("name").unwrap_or_default();
        app.profile_status.set(format!("已提交: {name}"));
    }),
);
command.execute_with_context(app, ctx);"#;

pub(crate) fn page(app: &App) -> Element<App> {
    demo_section::page(
        "Forms",
        "表单页面展示输入控件、选择控件、校验状态和受控值绑定。",
        vec![
            input_component(app),
            textarea_component(app),
            switch_component(app),
            checkbox_component(app),
            radio_component(app),
            select_component(app),
            combobox_component(app),
            slider_component(app),
            rating_component(app),
            date_picker_component(app),
            time_picker_component(app),
            number_input_component(app),
            color_picker_component(app),
            upload_component(app),
            form_component(app),
        ],
    )
}

fn input_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Input",
        "Input 由 TextController 驱动，可用于普通文本、路径或带校验的字段。",
        vec![
            UsageDemo::new(
                "input/basic",
                "受控输入",
                "此输入框也会被媒体页面的音频示例复用。",
                Input::new(app.input_text.clone())
                    .width(dp(320.0))
                    .placeholder("输入音频路径或 URL"),
                CODE_INPUT_BASIC,
            ),
            UsageDemo::new(
                "input/validation",
                "校验态输入",
                "Form 字段的验证状态可以直接驱动 Input 视觉反馈。",
                Flex::vertical().gap(dp(6.0)).child(el![
                    Input::new(app.profile_email.controller())
                        .width(dp(320.0))
                        .placeholder("name@example.com")
                        .validation(app.profile_email.validation_state()),
                    Text::new(
                        app.profile_email
                            .first_error()
                            .map(|v| v.unwrap_or_default())
                    )
                    .style_full(styles::status_style),
                ]),
                CODE_INPUT_VALIDATION,
            ),
        ],
    )
}

fn textarea_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Textarea",
        "Textarea 支持多行文本输入，并复用文本选择、光标和滚动基础设施。",
        vec![
            UsageDemo::new(
                "textarea/basic",
                "多行输入",
                "固定尺寸的多行文本区域适合备注和长内容。",
                Textarea::new(app.textarea_text.clone())
                    .size(dp(340.0), dp(140.0))
                    .placeholder("请输入多行内容"),
                CODE_TEXTAREA_BASIC,
            ),
            UsageDemo::new(
                "textarea/status",
                "变更回调",
                "on_change 可以把输入变化同步到 ViewModel 状态。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Textarea::new(app.textarea_text.clone())
                        .size(dp(340.0), dp(110.0))
                        .on_change(Command::new(|app: &mut App| {
                            app.profile_status.set("Textarea 已更新".to_string());
                        })),
                    Text::new(app.profile_status.signal()).style_full(styles::status_style),
                ]),
                CODE_TEXTAREA_STATUS,
            ),
        ],
    )
}

fn switch_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Switch",
        "Switch 表达二元开关，适合即时设置项。",
        vec![
            UsageDemo::new(
                "switch/basic",
                "受控开关",
                "开关状态来自 App::switch，并在变更时写回。",
                Flex::horizontal().gap(dp(10.0)).child(el![
                    Switch::new(app.switch.signal()).on_change(ValueCommand::new(
                        |app: &mut App, enabled| app.switch.set(enabled),
                    )),
                    Text::new(app.switch.signal().map(|enabled| {
                        if enabled { "已开启" } else { "已关闭" }.to_string()
                    }))
                    .style_full(styles::status_style),
                ]),
                CODE_SWITCH_BASIC,
            ),
            UsageDemo::new(
                "switch/disabled",
                "静态和禁用",
                "静态值可以用于只读预览，禁用态保留当前视觉状态。",
                Flex::horizontal().gap(dp(10.0)).child(el![
                    Switch::<App>::new(true),
                    Switch::<App>::new(false).disable(true),
                ]),
                CODE_SWITCH_DISABLED,
            ),
        ],
    )
}

fn checkbox_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Checkbox",
        "Checkbox 适合独立布尔选项，也可以参与 Form 校验。",
        vec![
            UsageDemo::new(
                "checkbox/basic",
                "带标签复选框",
                "点击标签或控件都会派发 on_change。",
                Checkbox::new(app.checkbox.signal())
                    .label("接收通知")
                    .on_change(ValueCommand::new(|app: &mut App, checked| {
                        app.checkbox.set(checked)
                    })),
                CODE_CHECKBOX_BASIC,
            ),
            UsageDemo::new(
                "checkbox/validation",
                "表单校验",
                "FormField<bool> 可直接提供 validation 和 bind_change。",
                Flex::vertical().gap(dp(6.0)).child(el![
                    Checkbox::new(app.profile_newsletter.signal())
                        .label("订阅每周邮件")
                        .validation(app.profile_newsletter.validation_state())
                        .on_change(app.profile_newsletter.bind_change()),
                    Text::new(
                        app.profile_newsletter
                            .first_error()
                            .map(|v| v.unwrap_or_default())
                    )
                    .style_full(styles::status_style),
                ]),
                CODE_CHECKBOX_VALIDATION,
            ),
        ],
    )
}

fn radio_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Radio / RadioGroup",
        "Radio 用于单项选择，RadioGroup 负责一组选项的互斥状态。",
        vec![
            UsageDemo::new(
                "radio/basic",
                "单个 Radio",
                "单独 Radio 可作为简单开关，也可以组成自定义分组。",
                Radio::new(app.radio.signal())
                    .label("单个单选框")
                    .on_change(ValueCommand::new(|app: &mut App, checked| {
                        app.radio.set(checked)
                    })),
                CODE_RADIO_BASIC,
            ),
            UsageDemo::new(
                "radio/group",
                "主题模式选择",
                "选择项会同步更新 demo 的主题模式。",
                RadioGroup::new(
                    vec![
                        RadioOption::new("system".to_string(), "跟随系统".to_string()),
                        RadioOption::new("light".to_string(), "明亮".to_string()),
                        RadioOption::new("dark".to_string(), "暗色".to_string()),
                    ],
                    app.contact_method.signal(),
                )
                .horizontal()
                .on_change(ValueCommand::new(|app: &mut App, (key, _label)| {
                    if key == "system" {
                        app.theme.set(ThemeMode::System);
                    } else if key == "light" {
                        app.theme.set(ThemeMode::Light);
                    } else {
                        app.theme.set(ThemeMode::Dark);
                    }
                    app.contact_method.set(key);
                })),
                CODE_RADIO_GROUP,
            ),
        ],
    )
}

fn select_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Select",
        "Select 用于从较短选项集合中选择一个值，支持禁用选项和 placeholder。",
        vec![
            UsageDemo::new(
                "select/basic",
                "基础选择",
                "选择结果写回 select_action 状态。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Select::new(
                        vec![
                            SelectOption::new("archive".to_string(), "归档".to_string()),
                            SelectOption::new("delete".to_string(), "删除".to_string())
                                .disable(true),
                            SelectOption::new("share".to_string(), "分享".to_string()),
                        ],
                        app.select_action.signal(),
                    )
                    .placeholder("请选择操作")
                    .width(dp(240.0))
                    .on_change(ValueCommand::new(
                        |app: &mut App, (key, _label)| {
                            app.select_action.set(Some(key));
                        }
                    )),
                    Text::new(app.select_action.signal().map(|value| {
                        format!("当前选择: {}", value.unwrap_or_else(|| "无".to_string()))
                    }))
                    .style_full(styles::status_style),
                ]),
                CODE_SELECT_BASIC,
            ),
            UsageDemo::new(
                "select/disabled",
                "禁用选项",
                "不可用操作可以留在菜单里但禁用点击。",
                Text::new("示例中的删除选项已禁用。").style_full(styles::status_style),
                CODE_SELECT_DISABLED,
            ),
        ],
    )
}

fn combobox_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Combobox / AutoComplete",
        "Combobox 组合输入框和本地选项过滤，AutoComplete 复用同一 API。",
        vec![UsageDemo::new(
            "combobox/local-filter",
            "本地搜索选择",
            "输入文本会对本地 options 做大小写不敏感过滤。",
            Flex::vertical().gap(dp(8.0)).child(el![
                Combobox::new(app.combobox_text.clone(), combo_options())
                    .open(app.combobox_open.signal())
                    .selected_key(app.combobox_selected.signal())
                    .placeholder("Search component")
                    .on_open_change(ValueCommand::new(|app: &mut App, open| {
                        app.combobox_open.set(open);
                    }))
                    .on_change(ValueCommand::new(|app: &mut App, change: ComboboxChange| {
                        app.combobox_selected.set(change.selected_key.clone());
                        app.component_status.set(format!("Combobox: {}", change.text));
                    })),
                Text::new(app.component_status.signal()).style_full(styles::status_style),
            ]),
            CODE_COMBOBOX_BASIC,
        )],
    )
}

fn slider_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Slider",
        "Slider 适合连续或步进数值，支持刻度和值标签。",
        vec![
            UsageDemo::new(
                "slider/basic",
                "带刻度和值标签",
                "当前值以百分比格式展示。",
                Slider::new(app.slider_value.signal(), 0.0, 100.0)
                    .width(dp(300.0))
                    .step(5.0)
                    .show_ticks(true)
                    .show_value_label(true)
                    .format_value(|value| format!("{value:.0}%")),
                CODE_SLIDER_BASIC,
            ),
            UsageDemo::new(
                "slider/controlled",
                "控制音量",
                "同一个值用于 UI 展示，也同步到音频控制器音量。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Slider::new(app.slider_value.signal(), 0.0, 100.0)
                        .width(dp(300.0))
                        .step(5.0)
                        .show_value_label(true)
                        .on_change(ValueCommand::new(|app: &mut App, value| {
                            app.slider_value.set(value);
                            app.audio_controller.set_volume(value / 100.0);
                        })),
                    Text::new(
                        app.slider_value
                            .signal()
                            .map(|value| format!("音量: {value:.0}%"))
                    )
                    .style_full(styles::status_style),
                ]),
                CODE_SLIDER_CONTROLLED,
            ),
            UsageDemo::new(
                "slider/vertical",
                "竖向滑块",
                "底部为最小值，顶部为最大值，适合音量、亮度等纵向控制。",
                Flex::horizontal().gap(dp(16.0)).align(Align::Center).child(el![
                    Slider::new(app.slider_value.signal(), 0.0, 100.0)
                        .vertical()
                        .height(dp(180.0))
                        .step(5.0)
                        .show_ticks(true)
                        .show_value_label(true)
                        .format_value(|value| format!("{value:.0}%"))
                        .on_change(ValueCommand::new(|app: &mut App, value| {
                            app.slider_value.set(value);
                        })),
                    Text::new(
                        app.slider_value
                            .signal()
                            .map(|value| format!("当前值: {value:.0}%"))
                    )
                    .style_full(styles::status_style),
                ]),
                CODE_SLIDER_VERTICAL,
            ),
        ],
    )
}

fn rating_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Rating",
        "Rating 适合评分输入和只读评分展示，支持半星步长。",
        vec![UsageDemo::new(
            "rating/half-step",
            "半星评分",
            "点击或键盘调整评分，变更会写回 ViewModel。",
            Flex::vertical().gap(dp(8.0)).child(el![
                Rating::new(app.rating_value.signal()).half().on_change(ValueCommand::new(
                    |app: &mut App, change: RatingChange| {
                        app.rating_value.set(change.value);
                        app.component_status.set(format!("评分: {:.1}", change.value));
                    },
                )),
                Text::new(app.component_status.signal()).style_full(styles::status_style),
            ]),
            CODE_RATING_BASIC,
        )],
    )
}

fn date_picker_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "DatePicker",
        "DatePicker 复用 TextController 与 Popover，支持直接输入和日历选择。",
        vec![UsageDemo::new(
            "date-picker/basic",
            "日期选择",
            "输入框和日历网格共享同一个受控日期状态。",
            Flex::vertical().gap(dp(8.0)).child(el![
                DatePicker::new(
                    app.demo_date_text.clone(),
                    app.demo_date.signal(),
                    app.demo_date_month.signal(),
                )
                .open(app.demo_date_open.signal())
                .on_open_change(ValueCommand::new(App::set_demo_date_open))
                .on_month_change(ValueCommand::new(App::set_demo_date_month))
                .on_change(ValueCommand::new(App::set_demo_date)),
                Text::new(app.profile_status.signal()).style_full(styles::status_style),
            ]),
            CODE_DATE_PICKER,
        )],
    )
}

fn time_picker_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "TimePicker",
        "TimePicker 提供可输入文本框和闹钟式时间弹层，可自由设置小时和分钟。",
        vec![UsageDemo::new(
            "time-picker/basic",
            "时间选择",
            "示例使用 24 小时制，可逐项调整小时和分钟并回写 ViewModel。",
            Flex::vertical().gap(dp(8.0)).child(el![
                TimePicker::new(app.demo_time_text.clone(), app.demo_time.signal())
                    .open(app.demo_time_open.signal())
                    .on_open_change(ValueCommand::new(App::set_demo_time_open))
                    .on_change(ValueCommand::new(App::set_demo_time)),
                Text::new(app.profile_status.signal()).style_full(styles::status_style),
            ]),
            CODE_TIME_PICKER,
        )],
    )
}

fn number_input_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "NumberInput",
        "NumberInput 组合文本输入和步进按钮，支持范围限制和解析状态。",
        vec![UsageDemo::new(
            "number-input/basic",
            "数字输入",
            "可直接输入数字，也可以用两侧按钮按步进调整。",
            Flex::vertical().gap(dp(8.0)).child(el![
                NumberInput::new(app.demo_number_text.clone(), app.demo_number.signal())
                    .range(0.0, 99.0)
                    .step(1.0)
                    .on_change(ValueCommand::new(App::set_demo_number)),
                Text::new(app.profile_status.signal()).style_full(styles::status_style),
            ]),
            CODE_NUMBER_INPUT,
        )],
    )
}

fn color_picker_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "ColorPicker",
        "ColorPicker 用输入控件风格的触发器展示当前颜色，并在 Popover 中调整色板和通道。",
        vec![UsageDemo::new(
            "color-picker/basic",
            "颜色选择",
            "点击色块打开颜色面板，选择预设色或拖动 RGBA 通道。",
            Flex::vertical().gap(dp(8.0)).child(el![
                ColorPicker::new(app.demo_color.signal())
                    .open(app.demo_color_open.signal())
                    .on_open_change(ValueCommand::new(App::set_demo_color_open))
                    .on_change(ValueCommand::new(App::set_demo_color)),
                Text::new(app.profile_status.signal()).style_full(styles::status_style),
            ]),
            CODE_COLOR_PICKER,
        )],
    )
}

fn upload_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Upload",
        "Upload 管理受控文件队列；选择、拖放、删除、错误和进度状态都由组件展示。",
        vec![UsageDemo::new(
            "upload/queue",
            "文件队列",
            "点击选择文件，或把文件拖入 drop zone；示例只模拟队列，不执行 HTTP 上传。",
            Flex::vertical().gap(dp(8.0)).child(el![
                Upload::new(app.upload_files.signal())
                    .accept_extensions(&["png", "jpg", "pdf", "txt"])
                    .max_files(8)
                    .on_select(ValueCommand::new(App::add_upload_files))
                    .on_remove(ValueCommand::new(App::remove_upload_file)),
                Flex::horizontal().gap(dp(8.0)).child(el![
                    Button::new("推进进度")
                        .secondary()
                        .on_click(Command::new(App::advance_uploads)),
                    Text::new(app.upload_status.signal()).style_full(styles::status_style),
                ]),
            ]),
            CODE_UPLOAD,
        )],
    )
}

fn form_component(app: &App) -> Element<App> {
    let name_validation = app.profile_name.validation_state();
    let email_validation = app.profile_email.validation_state();
    let newsletter_validation = app.profile_newsletter.validation_state();

    demo_section::component_doc(
        app,
        "Form",
        "Form 统一管理字段值、同步校验、异步校验和提交状态。",
        vec![
            UsageDemo::new(
                "form/fields",
                "字段和校验态",
                "字段错误会直接反馈到对应控件。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Input::new(app.profile_name.controller())
                        .placeholder("请输入名称")
                        .width(dp(300.0))
                        .validation(name_validation),
                    Text::new(
                        app.profile_name
                            .first_error()
                            .map(|v| v.unwrap_or_default())
                    )
                    .style_full(styles::status_style),
                    Input::new(app.profile_email.controller())
                        .placeholder("name@example.com")
                        .width(dp(300.0))
                        .validation(email_validation),
                    Text::new(
                        app.profile_email
                            .first_error()
                            .map(|v| v.unwrap_or_default())
                    )
                    .style_full(styles::status_style),
                    Checkbox::new(app.profile_newsletter.signal())
                        .label("订阅每周邮件")
                        .validation(newsletter_validation)
                        .on_change(app.profile_newsletter.bind_change()),
                ]),
                CODE_FORM_FIELDS,
            ),
            UsageDemo::new(
                "form/submit",
                "验证和提交",
                "按钮会触发异步验证、提交或重置。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                        Button::new("验证").on_click(Command::new_with_context(
                            |app: &mut App, ctx| {
                                let command = app.profile_form.validate_async_command::<App>();
                                command.execute_with_context(app, ctx);
                            },
                        )),
                        Button::new("提交")
                            .primary()
                            .on_click(Command::new_with_context(|app: &mut App, ctx| {
                                let form = app.profile_form.clone();
                                let command = form.submit_async_command(ValueCommand::new(
                                    |app: &mut App, snapshot: FormSnapshot| {
                                        let name =
                                            snapshot.get::<String>("name").unwrap_or_default();
                                        let email =
                                            snapshot.get::<String>("email").unwrap_or_default();
                                        app.profile_status.set(format!("已提交: {name} / {email}"));
                                    },
                                ));
                                command.execute_with_context(app, ctx);
                            },)),
                        Button::new("重置")
                            .ghost()
                            .on_click(Command::new(|app: &mut App| {
                                app.profile_form.reset();
                                app.profile_status.set("表单已重置".to_string());
                            })),
                    ]),
                    Text::new(app.profile_form.status().map(|status| {
                        format!(
                            "validating={}, submitting={}",
                            status.validating, status.submitting
                        )
                    }))
                    .style_full(styles::status_style),
                    Text::new(app.profile_form.is_valid().map(|valid| {
                        if valid {
                            "表单当前无错误"
                        } else {
                            "表单当前存在错误"
                        }
                        .to_string()
                    }))
                    .style_full(styles::status_style),
                    Text::new(app.profile_status.signal()).style_full(styles::status_style),
                ]),
                CODE_FORM_SUBMIT,
            ),
        ],
    )
}

fn combo_options() -> Vec<ComboboxOption> {
    vec![
        ComboboxOption::new("badge", "Badge"),
        ComboboxOption::new("avatar", "Avatar"),
        ComboboxOption::new("rich-text", "RichText"),
        ComboboxOption::new("combobox", "Combobox"),
        ComboboxOption::new("splitter", "Splitter"),
        ComboboxOption::new("carousel", "Carousel"),
    ]
}
