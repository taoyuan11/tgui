mod pages;

use tgui::{
    dp, sp, Application, Binding, Button, Color, Column, Command, Insets, MediaSource, Observable,
    Overflow, Row, ScrollbarStyle, Stack, Text, TguiError, ViewModelContext,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShowcasePage {
    Basic,
    Media,
    Canvas,
}

impl ShowcasePage {
    fn label(self) -> &'static str {
        match self {
            Self::Basic => "Basic Widgets",
            Self::Media => "Media Widgets",
            Self::Canvas => "Canvas",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Basic => "Text, buttons, input, layout containers, and simple reactive state.",
            Self::Media => "Image loading from bytes and paths, plus media event feedback.",
            Self::Canvas => {
                "Custom drawing with gradients, dashed strokes, shadows, and hit events."
            }
        }
    }
}

pub(crate) struct ShowcaseVm {
    pub(crate) current_page: Observable<ShowcasePage>,
    pub(crate) clicks: Observable<u32>,
    pub(crate) draft: Observable<String>,
    pub(crate) media_source: Observable<MediaSource>,
    pub(crate) media_status: Observable<String>,
    pub(crate) canvas_hover: Observable<String>,
    pub(crate) canvas_clicked: Observable<String>,
}

impl ShowcaseVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            current_page: ctx.observable(ShowcasePage::Basic),
            clicks: ctx.observable(0),
            draft: ctx.observable("Ship a polished multi-page example".to_string()),
            media_source: ctx.observable(pages::media::default_preview_source()),
            media_status: ctx.observable(
                "Click a preview button to switch between raster and SVG media.".to_string(),
            ),
            canvas_hover: ctx.observable("Move over a canvas path".to_string()),
            canvas_clicked: ctx.observable("Clicked item: none".to_string()),
        }
    }

    fn title(&self) -> Binding<String> {
        let current_page = self.current_page.clone();
        Binding::new(move || format!("tgui multi-page showcase - {}", current_page.get().label()))
    }

    fn page_label(&self) -> Binding<String> {
        let current_page = self.current_page.clone();
        Binding::new(move || current_page.get().label().to_string())
    }

    fn page_subtitle(&self) -> Binding<String> {
        let current_page = self.current_page.clone();
        Binding::new(move || current_page.get().subtitle().to_string())
    }

    fn show_basic(&mut self) {
        self.current_page.set(ShowcasePage::Basic);
    }

    fn show_media(&mut self) {
        self.current_page.set(ShowcasePage::Media);
    }

    fn show_canvas(&mut self) {
        self.current_page.set(ShowcasePage::Canvas);
    }

    pub(crate) fn increment_clicks(&mut self) {
        self.clicks.update(|clicks| *clicks += 1);
    }

    pub(crate) fn reset_clicks(&mut self) {
        self.clicks.set(0);
    }

    pub(crate) fn set_draft(&mut self, value: String) {
        self.draft.set(value);
    }

    pub(crate) fn preview_embedded_raster(&mut self) {
        self.media_status
            .set("Loading embedded raster preview...".to_string());
        self.media_source
            .set(pages::media::embedded_raster_source());
    }

    pub(crate) fn preview_embedded_svg(&mut self) {
        self.media_status
            .set("Loading embedded SVG preview...".to_string());
        self.media_source.set(pages::media::embedded_svg_source());
    }

    pub(crate) fn note_media_loading(&mut self) {
        self.media_status
            .set("Media widget is loading...".to_string());
    }

    pub(crate) fn note_media_ready(&mut self) {
        self.media_status
            .set("Media widget loaded successfully.".to_string());
    }

    pub(crate) fn note_media_error(&mut self, error: String) {
        self.media_status.set(format!("Media error: {error}"));
    }

    pub(crate) fn note_canvas_hover(&mut self, event: tgui::CanvasPointerEvent) {
        self.canvas_hover.set(format!(
            "Hover item={} canvas=({:.0}, {:.0}) local=({:.0}, {:.0})",
            event.item_id.get(),
            event.canvas_position.x,
            event.canvas_position.y,
            event.local_position.x,
            event.local_position.y
        ));
    }

    pub(crate) fn note_canvas_click(&mut self, event: tgui::CanvasPointerEvent) {
        self.canvas_clicked
            .set(format!("Clicked item: {}", event.item_id.get()));
    }

    fn page_content(&self) -> tgui::Element<Self> {
        match self.current_page.get() {
            ShowcasePage::Basic => pages::basic::view(self),
            ShowcasePage::Media => pages::media::view(self),
            ShowcasePage::Canvas => pages::canvas::view(self),
        }
    }

    fn view(&self) -> tgui::Element<Self> {
        Column::new()
            .fill_size()
            .padding(Insets::all(dp(24.0)))
            .gap(dp(18.0))
            .background(Color::hexa(0x06131FFF))
            .child(hero_panel(self.page_label(), self.page_subtitle()))
            .child(
                Row::new()
                    .gap(dp(10.0))
                    .child(tab_button(
                        "Basic Widgets",
                        self.current_page.get() == ShowcasePage::Basic,
                        Command::new(Self::show_basic),
                    ))
                    .child(tab_button(
                        "Media Widgets",
                        self.current_page.get() == ShowcasePage::Media,
                        Command::new(Self::show_media),
                    ))
                    .child(tab_button(
                        "Canvas",
                        self.current_page.get() == ShowcasePage::Canvas,
                        Command::new(Self::show_canvas),
                    )),
            )
            .child(
                Stack::new()
                    .fill_size()
                    .padding(Insets::all(dp(20.0)))
                    .background(Color::hexa(0x0B1B2BFF))
                    .border(dp(1.0), Color::hexa(0x24435DFF))
                    .border_radius(dp(24.0))
                    .overflow_y(Overflow::Scroll)
                    .scrollbar_style(
                        ScrollbarStyle::default()
                            .thumb_color(Color::hexa(0x4EA8DECC))
                            .hover_thumb_color(Color::hexa(0x89C2D9FF))
                            .insets(Insets::all(dp(8.0))),
                    )
                    .child(self.page_content()),
            )
            .into()
    }
}

fn hero_panel(title: Binding<String>, subtitle: Binding<String>) -> tgui::Element<ShowcaseVm> {
    Stack::new()
        .padding(Insets::all(dp(24.0)))
        .background(Color::hexa(0x0E2A47FF))
        .border_radius(dp(24.0))
        .child(
            Column::new()
                .gap(dp(10.0))
                .child(
                    Text::new("tgui multi-page showcase")
                        .font_size(sp(30.0))
                        .color(Color::hexa(0xF8FBFFFF)),
                )
                .child(
                    Text::new(title)
                        .font_size(sp(20.0))
                        .color(Color::hexa(0x9BD1FFFF)),
                )
                .child(
                    Text::new(subtitle)
                        .font_size(sp(15.0))
                        .color(Color::hexa(0xD7ECFFFF)),
                ),
        )
        .into()
}

fn tab_button(
    label: &str,
    active: bool,
    command: Command<ShowcaseVm>,
) -> tgui::Element<ShowcaseVm> {
    Button::new(Text::new(label).color(if active {
        Color::hexa(0x08111BFF)
    } else {
        Color::hexa(0xDCEFFDFF)
    }))
    .padding(Insets::symmetric(dp(16.0), dp(10.0)))
    .background(if active {
        Color::hexa(0x7DD3FCFF)
    } else {
        Color::hexa(0x12314DFF)
    })
    .border(dp(1.0), Color::hexa(0x2E5877FF))
    .border_radius(dp(999.0))
    .on_click(command)
    .into()
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .window_size(dp(1200.0), dp(900.0))
        .with_view_model(ShowcaseVm::new)
        .bind_title(ShowcaseVm::title)
        .root_view(ShowcaseVm::view)
        .run()
}
