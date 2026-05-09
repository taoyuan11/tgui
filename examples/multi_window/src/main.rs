use tgui::prelude::*;

fn stateful<T: Clone>(value: T) -> Stateful<T> {
    Stateful {
        normal: value.clone(),
        hovered: value.clone(),
        pressed: value.clone(),
        disabled: value,
    }
}

fn text_style(mode: ResolvedThemeMode, size: Sp, color: Color) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.typography.size = size;
    style.color = color.into();
    style
}

fn panel_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.background = Some(Color::hexa(0x162033F0).into());
    style.surface.border_color = Some(Color::hexa(0x2E4262FF).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(18.0).into());
    style
}

fn filled_button_style(mode: ResolvedThemeMode, color: Color) -> ButtonStyle {
    ButtonStyle {
        surface: WidgetSurfaceStyle::default(),
        background: stateful(color.into()),
        foreground: stateful(Color::WHITE.into()),
        border: stateful(Color::TRANSPARENT.into()),
        focus_ring: None,
        border_width: dp(0.0).into(),
        radius: dp(12.0).into(),
        padding_x: dp(16.0),
        padding_y: dp(10.0),
        min_height: dp(40.0),
        text_style: TextWidgetStyle::default_for(mode).typography,
    }
}

struct MultiWindowVm {
    next_document_id: State<u32>,
    inspector_open: State<bool>,
    documents: State<Vec<u32>>,
}

impl MultiWindowVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            next_document_id: ctx.state(3),
            inspector_open: ctx.state(true),
            documents: ctx.state(vec![1, 2]),
        }
    }

    fn main_title(&self) -> Signal<String> {
        self.documents
            .signal()
            .map(|documents| format!("tgui multi-window - {} document windows", documents.len()))
    }

    fn inspector_title(&self) -> Signal<String> {
        self.inspector_open
            .signal()
            .map(|_| "Inspector".to_string())
    }

    fn document_title(&self, id: u32) -> Signal<String> {
        self.documents.signal().map(move |documents| {
            if documents.contains(&id) {
                format!("Document {id}")
            } else {
                format!("Document {id} (hidden)")
            }
        })
    }

    fn document_summary(&self) -> Signal<String> {
        self.documents.signal().map(|documents| {
            if documents.is_empty() {
                "No registered document windows.".to_string()
            } else {
                format!("Registered document ids: {:?}", documents)
            }
        })
    }

    fn toggle_inspector(&mut self) {
        self.inspector_open.update(|is_open| *is_open = !*is_open);
    }

    fn open_document(&mut self) {
        let next_id = self.next_document_id.get();
        self.documents.update(|documents| documents.push(next_id));
        self.next_document_id.set(next_id + 1);
    }

    fn close_last_document(&mut self) {
        self.documents.update(|documents| {
            documents.pop();
        });
    }

    fn main_view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(28.0)))
            .center()
            .child(
                Flex::new(Axis::Vertical)
                    .width(dp(620.0))
                    .padding(Insets::all(dp(24.0)))
                    .gap(dp(14.0))
                    .style(panel_style)
                    .child(
                        Text::new("Multi-window runtime")
                            .style(|mode| text_style(mode, sp(28.0), Color::hexa(0xF8FAFCFF))),
                    )
                    .child(
                        Text::new(
                            "This example keeps one shared view model while dynamically reconciling a main window, an optional inspector, and multiple document windows.",
                        )
                        .style(|mode| text_style(mode, sp(15.0), Color::hexa(0xCBD5E1FF))),
                    )
                    .child(
                        Text::new(self.document_summary())
                            .style(|mode| text_style(mode, sp(15.0), Color::hexa(0x7DD3FCFF))),
                    )
                    .child(
                        Flex::new(Axis::Horizontal)
                            .gap(dp(10.0))
                            .child(
                                Button::new("Toggle inspector")
                                    .grow(1.0)
                                    .style(|mode| filled_button_style(mode, Color::hexa(0x0F766EFF)))
                                    .on_click(Command::new(Self::toggle_inspector)),
                            )
                            .child(
                                Button::new("Spawn document")
                                    .grow(1.0)
                                    .style(|mode| filled_button_style(mode, Color::hexa(0x1D4ED8FF)))
                                    .on_click(Command::new(Self::open_document)),
                            )
                            .child(
                                Button::new("Remove last")
                                    .grow(1.0)
                                    .style(|mode| filled_button_style(mode, Color::hexa(0x7C2D12FF)))
                                    .on_click(Command::new(Self::close_last_document)),
                            ),
                    ),
            )
            .into()
    }

    fn inspector_view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(18.0)))
            .child(
                Flex::new(Axis::Vertical)
                    .gap(dp(10.0))
                    .child(
                        Text::new("Inspector")
                            .style(|mode| text_style(mode, sp(24.0), Color::hexa(0xE2E8F0FF))),
                    )
                    .child(
                        Text::new(self.document_summary())
                            .style(|mode| text_style(mode, sp(15.0), Color::hexa(0x93C5FDFF))),
                    )
                    .child(
                        Text::new(
                            "Close this window from the button in the main window, or use the native close button to hide just this instance.",
                        )
                        .style(|mode| text_style(mode, sp(14.0), Color::hexa(0xCBD5E1FF))),
                    ),
            )
            .into()
    }

    fn document_view(&self, id: u32) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(20.0)))
            .child(
                Flex::new(Axis::Vertical)
                    .gap(dp(12.0))
                    .child(el![
                        Text::new(self.document_title(id))
                            .style(|mode| text_style(mode, sp(26.0), Color::hexa(0xF8FAFCFF))),
                        Text::new(
                            self.documents
                                .signal()
                                .map(move |documents| format!("Shared registry size: {}", documents.len())),
                        )
                        .style(|mode| text_style(mode, sp(15.0), Color::hexa(0x93C5FDFF))),
                        Text::new(
                            "Each document window owns its own renderer, focus state, scroll state, and animation state, but still reads from the same shared view model.",
                        )
                        .style(|mode| text_style(mode, sp(14.0), Color::hexa(0xCBD5E1FF))),
                        Button::new("Close")
                            .danger()
                            .style(|mode| filled_button_style(mode, Color::hexa(0x991B1BFF)))
                            .on_click(Command::new_with_context(|_, context| {
                                context.window().close()
                            })),
                    ])
            )
            .into()
    }

    fn windows(&self) -> Vec<WindowSpec<Self>> {
        let mut windows = vec![WindowSpec::main("main")
            .title("tgui multi-window")
            .window_size(dp(980.0), dp(700.0))
            .min_window_size(dp(760.0), dp(520.0))
            .max_window_size(dp(1280.0), dp(900.0))
            .bind_title(Self::main_title)
            .root_view(Self::main_view)];

        if self.inspector_open.get() {
            windows.push(
                WindowSpec::child("inspector")
                    .title("Inspector")
                    .window_size(dp(420.0), dp(320.0))
                    .min_window_size(dp(320.0), dp(240.0))
                    .max_window_size(dp(640.0), dp(480.0))
                    .bind_title(Self::inspector_title)
                    .root_view(Self::inspector_view),
            );
        }

        for id in self.documents.get() {
            windows.push(
                WindowSpec::child(format!("document-{id}"))
                    .title(format!("Document {id}"))
                    .window_size(dp(540.0), dp(360.0))
                    .min_window_size(dp(360.0), dp(260.0))
                    .max_window_size(dp(900.0), dp(700.0))
                    .bind_title(move |vm: &Self| vm.document_title(id))
                    .root_view(move |vm: &Self| vm.document_view(id)),
            );
        }

        windows
    }
}

impl ViewModel for MultiWindowVm {
    fn new(ctx: &ViewModelContext) -> Self {
        MultiWindowVm::new(ctx)
    }

    fn view(&self) -> Element<Self> {
        MultiWindowVm::main_view(self)
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .close_children_with_main(true)
        .with_view_model(MultiWindowVm::new)
        .windows(MultiWindowVm::windows)
        .run()
}
