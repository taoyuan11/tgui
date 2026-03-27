use tgui::{Application, Binding, Column, FontWeight, Insets, Text, ViewModelContext};

struct WidgetDemoViewModel {
    clicks: tgui::Observable<u32>,
    input: tgui::Observable<String>,
}

impl WidgetDemoViewModel {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            clicks: context.observable(0),
            input: context.observable("Edit me".to_string()),
        }
    }

    fn title(&self) -> Binding<String> {
        let clicks = self.clicks.binding();
        let input = self.input.binding();
        Binding::new(move || {
            format!(
                "widgets - clicks: {} - input: {}",
                clicks.get(),
                input.get()
            )
        })
    }

    fn view(&self) -> tgui::Element<Self> {
        let clicks_text = self
            .clicks
            .binding()
            .map(|count| format!("点我点我: {count}"));

        let background = wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };

        Column::new()
            .padding(Insets::all(24.0))
            .gap(20.0)
            .child(
                Text::new('\u{e88a}'.to_string())
                    .font("icon")
                    .font_size(20.0)
                    .background(background)
                    .font_weight(FontWeight::SEMIBOLD),
            )
            .child(
                Text::new(clicks_text.clone())
                    .font_size(18.0)
                    .font("JetBrains Mono")
                    .background(background),
            )
            .child(
                Text::new(clicks_text.clone())
                    .font_size(18.0)
                    .background(background),
            )
            .child(
                Text::new("点我点我".to_string())
                    .font("楷体")
                    .font_size(18.0)
                    .background(background),
            )
            .child(
                Text::new("Hello World".to_string())
                    .font_size(18.0)
                    .background(background),
            )
            .into()
    }
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .window_size(960, 640)
        .font("icon", include_bytes!("./static/google-icon-font.ttf"))
        .with_view_model(WidgetDemoViewModel::new)
        .bind_title(WidgetDemoViewModel::title)
        .root_view(WidgetDemoViewModel::view)
        .run()
}
