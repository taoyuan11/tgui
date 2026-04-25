use tgui::{
    Align, Application, Axis, Binding, Button, Color, Command, Flex, Insets, Observable, Stack,
    Text, TguiError, ViewModelContext, WindowSpec, dp, pct, sp,
};

struct MultiWindowVm {
    next_document_id: Observable<u32>,
    inspector_open: Observable<bool>,
    documents: Observable<Vec<u32>>,
}

impl MultiWindowVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            next_document_id: ctx.observable(3),
            inspector_open: ctx.observable(true),
            documents: ctx.observable(vec![1, 2]),
        }
    }

    fn main_title(&self) -> Binding<String> {
        self.documents
            .binding()
            .map(|documents| format!("tgui multi-window - {} document windows", documents.len()))
    }

    fn inspector_title(&self) -> Binding<String> {
        self.inspector_open
            .binding()
            .map(|_| "Inspector".to_string())
    }

    fn document_title(&self, id: u32) -> Binding<String> {
        self.documents.binding().map(move |documents| {
            if documents.contains(&id) {
                format!("Document {id}")
            } else {
                format!("Document {id} (hidden)")
            }
        })
    }

    fn document_summary(&self) -> Binding<String> {
        self.documents.binding().map(|documents| {
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

    fn main_view(&self) -> tgui::Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(28.0)))
            .align(Align::Center)
            .child(
                Flex::new(Axis::Vertical)
                    .width(dp(620.0))
                    .padding(Insets::all(dp(24.0)))
                    .gap(dp(14.0))
                    .background(Color::hexa(0x162033F0))
                    .border(dp(1.0), Color::hexa(0x2E4262FF))
                    .border_radius(dp(18.0))
                    .child(
                        Text::new("Multi-window runtime")
                            .font_size(sp(28.0))
                            .color(Color::hexa(0xF8FAFCFF)),
                    )
                    .child(
                        Text::new(
                            "This example keeps one shared view model while dynamically reconciling a main window, an optional inspector, and multiple document windows.",
                        )
                        .font_size(sp(15.0))
                        .color(Color::hexa(0xCBD5E1FF)),
                    )
                    .child(
                        Text::new(self.document_summary())
                            .font_size(sp(15.0))
                            .color(Color::hexa(0x7DD3FCFF)),
                    )
                    .child(
                        Flex::new(Axis::Horizontal)
                            .gap(dp(10.0))
                            .child(
                                Button::new(Text::new("Toggle inspector"))
                                    .grow(1.0)
                                    .background(Color::hexa(0x0F766EFF))
                                    .border_radius(dp(12.0))
                                    .on_click(Command::new(Self::toggle_inspector)),
                            )
                            .child(
                                Button::new(Text::new("Spawn document"))
                                    .grow(1.0)
                                    .background(Color::hexa(0x1D4ED8FF))
                                    .border_radius(dp(12.0))
                                    .on_click(Command::new(Self::open_document)),
                            )
                            .child(
                                Button::new(Text::new("Remove last"))
                                    .grow(1.0)
                                    .background(Color::hexa(0x7C2D12FF))
                                    .border_radius(dp(12.0))
                                    .on_click(Command::new(Self::close_last_document)),
                            ),
                    ),
            )
            .into()
    }

    fn inspector_view(&self) -> tgui::Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(18.0)))
            .child(
                Flex::new(Axis::Vertical)
                    .gap(dp(10.0))
                    .child(
                        Text::new("Inspector")
                            .font_size(sp(24.0))
                            .color(Color::hexa(0xE2E8F0FF)),
                    )
                    .child(
                        Text::new(self.document_summary())
                            .font_size(sp(15.0))
                            .color(Color::hexa(0x93C5FDFF)),
                    )
                    .child(
                        Text::new(
                            "Close this window from the button in the main window, or use the native close button to hide just this instance.",
                        )
                        .font_size(sp(14.0))
                        .color(Color::hexa(0xCBD5E1FF)),
                    ),
            )
            .into()
    }

    fn document_view(&self, id: u32) -> tgui::Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(20.0)))
            .child(
                Flex::new(Axis::Vertical)
                    .gap(dp(12.0))
                    .child(
                        Text::new(self.document_title(id))
                            .font_size(sp(26.0))
                            .color(Color::hexa(0xF8FAFCFF)),
                    )
                    .child(
                        Text::new(
                            self.documents
                                .binding()
                                .map(move |documents| format!("Shared registry size: {}", documents.len())),
                        )
                        .font_size(sp(15.0))
                        .color(Color::hexa(0x93C5FDFF)),
                    )
                    .child(
                        Text::new(
                            "Each document window owns its own renderer, focus state, scroll state, and animation state, but still reads from the same shared view model.",
                        )
                        .font_size(sp(14.0))
                        .color(Color::hexa(0xCBD5E1FF)),
                    ),
            )
            .into()
    }

    fn windows(&self) -> Vec<WindowSpec<Self>> {
        let mut windows = vec![
            WindowSpec::main("main")
                .title("tgui multi-window")
                .window_size(dp(980.0), dp(700.0))
                .bind_title(Self::main_title)
                .root_view(Self::main_view),
        ];

        if self.inspector_open.get() {
            windows.push(
                WindowSpec::child("inspector")
                    .title("Inspector")
                    .window_size(dp(420.0), dp(320.0))
                    .bind_title(Self::inspector_title)
                    .root_view(Self::inspector_view),
            );
        }

        for id in self.documents.get() {
            windows.push(
                WindowSpec::child(format!("document-{id}"))
                    .title(format!("Document {id}"))
                    .window_size(dp(540.0), dp(360.0))
                    .bind_title(move |vm: &Self| vm.document_title(id))
                    .root_view(move |vm: &Self| vm.document_view(id)),
            );
        }

        windows
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .close_children_with_main(true)
        .with_view_model(MultiWindowVm::new)
        .windows(MultiWindowVm::windows)
        .run()
}
