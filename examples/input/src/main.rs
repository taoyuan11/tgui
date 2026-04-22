use tgui::{
    Align, Application, Binding, Button, Color, Column, Command, Input, Insets, Observable, Row,
    Stack, Text, TguiError, ValueCommand, ViewModelContext, dp, sp,
};

struct FormVm {
    project: Observable<String>,
    owner: Observable<String>,
    status: Observable<String>,
}

impl FormVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            project: ctx.observable(String::new()),
            owner: ctx.observable(String::new()),
            status: ctx.observable("Planning".to_string()),
        }
    }

    fn title(&self) -> Binding<String> {
        let project = self.project.clone();
        Binding::new(move || {
            let project = project.get();
            if project.trim().is_empty() {
                "tgui input form".to_string()
            } else {
                format!("tgui input form - {project}")
            }
        })
    }

    fn summary(&self) -> Binding<String> {
        let project = self.project.clone();
        let owner = self.owner.clone();
        let status = self.status.clone();
        Binding::new(move || {
            let project = readable(project.get(), "Untitled project");
            let owner = readable(owner.get(), "No owner assigned");
            let status = readable(status.get(), "No status");
            format!("Project: {project}\nOwner: {owner}\nStatus: {status}")
        })
    }

    fn set_project(&mut self, value: String) {
        self.project.set(value);
    }

    fn set_owner(&mut self, value: String) {
        self.owner.set(value);
    }

    fn set_status(&mut self, value: String) {
        self.status.set(value);
    }

    fn fill_demo(&mut self) {
        self.project.set("Cross-platform launcher".to_string());
        self.owner.set("Product Design".to_string());
        self.status.set("Ready for review".to_string());
    }

    fn clear(&mut self) {
        self.project.set(String::new());
        self.owner.set(String::new());
        self.status.set(String::new());
    }

    fn view(&self) -> tgui::Element<Self> {
        Stack::new()
            .fill_size()
            .padding(Insets::all(dp(24.0)))
            .align(Align::Center)
            .child(
                Column::new()
                    .width(dp(620.0))
                    .padding(Insets::all(dp(24.0)))
                    .gap(dp(14.0))
                    .background(Color::hexa(0x111827F2))
                    .border(dp(1.0), Color::hexa(0x334155FF))
                    .border_radius(dp(18.0))
                    .child(
                        Text::new("Reactive input form")
                            .font_size(sp(26.0))
                            .color(Color::hexa(0xF8FAFCFF)),
                    )
                    .child(
                        Text::new("Each input writes into an Observable<String>, and the summary card below updates immediately.")
                            .font_size(sp(15.0))
                            .color(Color::hexa(0xCBD5E1FF)),
                    )
                    .child(
                        Input::new(Text::new(self.project.binding()))
                            .fill_width()
                            .background(Color::hexa(0x1E293BFF))
                            .border(dp(1.0), Color::hexa(0x475569FF))
                            .border_radius(dp(12.0))
                            .placeholder_with_str("Project name")
                            .on_change(ValueCommand::new(Self::set_project)),
                    )
                    .child(
                        Input::new(Text::new(self.owner.binding()))
                            .fill_width()
                            .background(Color::hexa(0x1E293BFF))
                            .border(dp(1.0), Color::hexa(0x475569FF))
                            .border_radius(dp(12.0))
                            .placeholder_with_str("Owner or team")
                            .on_change(ValueCommand::new(Self::set_owner)),
                    )
                    .child(
                        Input::new(Text::new(self.status.binding()))
                            .fill_width()
                            .background(Color::hexa(0x1E293BFF))
                            .border(dp(1.0), Color::hexa(0x475569FF))
                            .border_radius(dp(12.0))
                            .placeholder_with_str("Status")
                            .on_change(ValueCommand::new(Self::set_status)),
                    )
                    .child(
                        Row::new()
                            .gap(dp(10.0))
                            .child(
                                Button::new(Text::new("Fill demo values"))
                                    .grow(1.0)
                                    .background(Color::hexa(0x0369A1FF))
                                    .border_radius(dp(12.0))
                                    .on_click(Command::new(Self::fill_demo)),
                            )
                            .child(
                                Button::new(Text::new("Clear"))
                                    .grow(1.0)
                                    .background(Color::hexa(0x7C2D12FF))
                                    .border_radius(dp(12.0))
                                    .on_click(Command::new(Self::clear)),
                            ),
                    )
                    .child(
                        Text::new(self.summary())
                            .padding(Insets::all(dp(16.0)))
                            .background(Color::hexa(0x0F172AFF))
                            .border(dp(1.0), Color::hexa(0x1D4ED8FF))
                            .border_radius(dp(14.0))
                            .color(Color::hexa(0xDBEAFEFF)),
                    ),
            )
            .into()
    }
}

fn readable(value: String, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .window_size(dp(980.0), dp(700.0))
        .with_view_model(FormVm::new)
        .bind_title(FormVm::title)
        .root_view(FormVm::view)
        .run()
}
