use tgui::prelude::*;

struct TextAreaVm {
    notes: Observable<String>,
    status: Observable<String>,
}

impl ViewModel for TextAreaVm {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            notes: context.observable(String::from(
                "多行文本域示例\n- 自动换行\n- 自动增高\n- 超过上限后内部滚动",
            )),
            status: context.observable(String::from("默认 Enter 只换行")),
        }
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(32.0)))
            .child(
                Flex::vertical()
                    .gap(dp(14.0))
                    .padding(Insets::all(dp(24.0)))
                    .width(dp(420.0))
                    .background(Color::hexa(0x132033FF))
                    .border(dp(1.0), Color::hexa(0x36506EFF))
                    .border_radius(dp(18.0))
                    .child(el![
                        Text::new("TextArea 示例")
                            .font_size(sp(26.0))
                            .color(Color::WHITE),
                        Text::new("受控多行文本域，单 Enter 提交，超过 max_rows 后内部滚动。")
                            .font_size(sp(14.0))
                            .color(Color::hexa(0xBDD0E6FF)),
                        TextArea::new(Text::new(self.notes.binding()))
                            .rows(4)
                            .min_rows(3)
                            .max_rows(6)
                            .submit_on_enter(true)
                            .placeholder_with_str("输入一些多行内容")
                            .on_change(ValueCommand::new(|vm: &mut TextAreaVm, text| {
                                vm.notes.set(text);
                                vm.status.set("编辑中，按 Enter 会触发提交".to_string());
                            }))
                            .on_submit(Command::new(|vm: &mut TextAreaVm| {
                                vm.status.set("已触发 on_submit".to_string());
                            })),
                        Text::new(self.status.binding())
                            .font_size(sp(13.0))
                            .color(Color::hexa(0x9FB8D3FF)),
                    ]),
            )
            .into()
    }
}

fn main() -> Result<(), TguiError> {
    let mut theme = Theme::dark();
    theme.colors.background = Color::hexa(0x09111DFF);
    theme.colors.surface = Color::hexa(0x102032FF);
    theme.colors.surface_low = Color::hexa(0x172A40FF);
    theme.colors.primary = Color::hexa(0x4AA8FFFF);

    Application::new()
        .title("tgui text area")
        .window_size(dp(960.0), dp(640.0))
        .theme(theme)
        .with_view_model(TextAreaVm::new)
        .root_view(TextAreaVm::view)
        .run()
}
