use std::path::Path;
use tgui::{Application, Color, Column, Command, Container, ContentFit, Image, Insets, Observable, Overflow, Row, Stack, Text, ValueCommand, Video, ViewModelContext};

struct MediaShowcaseVm {
    image_status: Observable<String>,
    video_status: Observable<String>,
}

impl MediaShowcaseVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            image_status: ctx.observable("waiting".to_string()),
            video_status: ctx.observable("waiting".to_string()),
        }
    }

    fn set_image_loading(&mut self) {
        self.image_status
            .update(|status| *status = "loading".to_string());
    }

    fn set_image_success(&mut self) {
        self.image_status
            .update(|status| *status = "success".to_string());
    }

    fn set_image_error(&mut self, error: String) {
        self.image_status
            .update(|status| *status = format!("error: {error}"));
    }

    fn set_video_loading(&mut self) {
        self.video_status
            .update(|status| *status = "loading".to_string());
    }

    fn set_video_success(&mut self) {
        self.video_status
            .update(|status| *status = "success".to_string());
    }

    fn set_video_error(&mut self, error: String) {
        self.video_status
            .update(|status| *status = format!("error: {error}"));
    }

    fn view(&self) -> tgui::Element<Self> {
        Column::new()
            .fill_size()
            .padding(Insets::all(24.0))
            .gap(18.0)
            .background(Color::hexa(0x020617FF))
            .overflow(Overflow::Scroll)
            .child(
                Stack::new()
                    .padding(Insets::all(20.0))
                    .background(Color::hexa(0x1D4ED8FF))
                    .border_radius(12.0)
                    .child(
                        Column::new()
                            .gap(8.0)
                            .child(
                                Text::new("Media showcase")
                                    .font_size(30.0)
                                    .color(Color::WHITE),
                            )
                            .child(
                                Text::new("Image and video both expose loading, success, and error events. Enable `video-ffmpeg` to decode video on desktop targets.")
                                    .font_size(15.0)
                                    .color(Color::hexa(0xDBEAFEFF)),
                            ),
                    ),
            )
            .child(
                Row::new()
                    .gap(18.0)
                    .child(card(
                        "Network image",
                        "Remote http/https resource with lifecycle events",
                        Column::new()
                            .gap(10.0)
                            .child(
                                Image::from_url("https://images.unsplash.com/photo-1500530855697-b586d89ba3ee?auto=format&fit=crop&w=1200&q=80")
                                    .height(250.0)
                                    .fill_width()
                                    .fit(ContentFit::Cover)
                                    .border_radius(10.0)
                                    .on_loading(Command::new(Self::set_image_loading))
                                    .on_success(Command::new(Self::set_image_success))
                                    .on_error(ValueCommand::new(Self::set_image_error)),
                            )
                            .child(
                                Text::new(
                                    self.image_status
                                        .binding()
                                        .map(|status| format!("image status: {status}")),
                                )
                                .font_size(12.0)
                                .color(Color::hexa(0x93C5FDFF)),
                            ),
                    ))
                    .child(card(
                        "Local image",
                        "Replace the path with one of your own files",
                        Image::from_path(
                            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/static/wlop.jpg")
                        )
                            .height(250.0)
                            .fill_width()
                            .fit(ContentFit::Contain)
                            .background(Color::hexa(0x0F172AFF))
                            .border_radius(10.0),
                    )),
            )
            .child(
                Row::new()
                    .gap(18.0)
                    .child(card(
                        "Network video",
                        "Enable `video-ffmpeg` for desktop decode and event callbacks",
                        Column::new()
                            .gap(10.0)
                            .child(
                                Video::from_url(
                                    "https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4",
                                )
                                .height(280.0)
                                .fill_width()
                                .fit(ContentFit::Contain)
                                .autoplay(true)
                                .border_radius(10.0)
                                .on_loading(Command::new(Self::set_video_loading))
                                .on_success(Command::new(Self::set_video_success))
                                .on_error(ValueCommand::new(Self::set_video_error)),
                            )
                            .child(
                                Text::new(
                                    self.video_status
                                        .binding()
                                        .map(|status| format!("video status: {status}")),
                                )
                                .font_size(12.0)
                                .color(Color::hexa(0xFCD34DFF)),
                            ),
                    ))
                    .child(card(
                        "Local video",
                        "Point this at a local `.mp4` or similar source",
                        Video::from_path(
                            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/static/flower.mp4")
                        )
                            .height(280.0)
                            .fill_width()
                            .fit(ContentFit::Contain)
                            .autoplay(true)
                            .volume(1.0)
                            .background(Color::hexa(0x0F172AFF))
                            .border_radius(10.0),
                    )),
            )
            .child(
                Container::new()
                    .padding(Insets::all(14.0))
                    .background(Color::hexa(0x111827FF))
                    .border(1.0, Color::hexa(0x334155FF))
                    .border_radius(12.0)
                    .child(
                        Text::new("Tip: set an explicit size or `aspect_ratio(...)` when you want media layout to stay stable before the resource finishes loading.")
                            .font_size(13.0)
                            .color(Color::hexa(0xCBD5E1FF)),
                    ),
            )
            .into()
    }
}

fn card<VM>(
    title: &str,
    subtitle: &str,
    body: impl Into<tgui::Element<VM>>,
) -> tgui::Element<VM> {
    Column::new()
        .grow(1.0)
        .padding(Insets::all(16.0))
        .gap(10.0)
        .background(Color::hexa(0x111827FF))
        .border(1.0, Color::hexa(0x334155FF))
        .border_radius(12.0)
        .child(Text::new(title).font_size(18.0).color(Color::WHITE))
        .child(
            Text::new(subtitle)
                .font_size(13.0)
                .color(Color::hexa(0xCBD5E1FF)),
        )
        .child(body)
        .into()
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .title("tgui media showcase")
        .window_size(1200, 820)
        .with_view_model(MediaShowcaseVm::new)
        .root_view(MediaShowcaseVm::view)
        .run()
}
