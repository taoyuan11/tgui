use std::path::PathBuf;

use crate::ShowcaseVm;
use tgui::{
    dp, pct, sp, Axis, Button, Color, Command, ContentFit, Flex, Image, Insets, MediaSource,
    Stack, Text,
};

const EMBEDDED_RASTER: &[u8] =
    include_bytes!("../../../image_example/src/static/juequling_shushu.jpg");
const EMBEDDED_SVG: &[u8] = br##"
<svg xmlns="http://www.w3.org/2000/svg" width="320" height="220" viewBox="0 0 320 220">
  <defs>
    <linearGradient id="bg" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#38bdf8" />
      <stop offset="100%" stop-color="#0f766e" />
    </linearGradient>
  </defs>
  <rect x="12" y="12" width="296" height="196" rx="28" fill="url(#bg)" />
  <circle cx="100" cy="96" r="38" fill="#f8fafc" fill-opacity="0.86" />
  <path d="M168 144 L228 64 L276 144 Z" fill="#ecfeff" fill-opacity="0.92" />
  <text x="30" y="188" fill="#ecfeff" font-size="28" font-family="Arial, sans-serif">
    Embedded SVG preview
  </text>
</svg>
"##;

pub(crate) fn embedded_raster_source() -> MediaSource {
    MediaSource::bytes(EMBEDDED_RASTER)
}

pub(crate) fn embedded_svg_source() -> MediaSource {
    MediaSource::bytes(EMBEDDED_SVG)
}

pub(crate) fn default_preview_source() -> MediaSource {
    embedded_raster_source()
}

fn local_svg_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../image_example/src/static/vvv.svg")
}

pub(crate) fn view(vm: &ShowcaseVm) -> tgui::Element<ShowcaseVm> {
    Flex::new(Axis::Vertical)
        .width(pct(100.0))
        .gap(dp(18.0))
        .child(
            Stack::new()
                .padding(Insets::all(dp(20.0)))
                .background(Color::hexa(0x123552FF))
                .border_radius(dp(20.0))
                .child(
                    Flex::new(Axis::Vertical)
                        .gap(dp(10.0))
                        .child(
                            Text::new("Page 2: media widgets")
                                .font_size(sp(26.0))
                                .color(Color::WHITE),
                        )
                        .child(
                            Text::new(
                                "This page focuses on Image and MediaSource. It shows loading from embedded bytes, loading from a local path, and reacting to media lifecycle events.",
                            )
                            .font_size(sp(15.0))
                            .color(Color::hexa(0xD6EFFF)),
                        ),
                ),
        )
        .child(
            Flex::new(Axis::Horizontal)
                .gap(dp(18.0))
                .child(
                    Stack::new().grow(1.0).child(media_card(
                        "Embedded raster",
                        "Image::from_bytes with a bundled JPEG asset.",
                        Image::from_bytes(EMBEDDED_RASTER)
                            .size(dp(320.0), dp(220.0))
                            .fit(ContentFit::Cover)
                            .background(Color::hexa(0x08111BFF))
                            .border_radius(dp(18.0))
                            .into(),
                    )),
                )
                .child(
                    Stack::new().grow(1.0).child(media_card(
                        "Local SVG path",
                        "Image::from_path resolves an SVG file from the examples directory.",
                        Image::from_path(local_svg_path())
                            .size(dp(320.0), dp(220.0))
                            .fit(ContentFit::Contain)
                            .background(Color::hexa(0x08111BFF))
                            .border_radius(dp(18.0))
                            .into(),
                    )),
                ),
        )
        .child(preview_panel(vm))
        .into()
}

fn preview_panel(vm: &ShowcaseVm) -> tgui::Element<ShowcaseVm> {
    Flex::new(Axis::Vertical)
        .padding(Insets::all(dp(18.0)))
        .gap(dp(14.0))
        .background(Color::hexa(0x0F2439FF))
        .border(dp(1.0), Color::hexa(0x264761FF))
        .border_radius(dp(18.0))
        .child(
            Text::new("Switchable preview")
                .font_size(sp(20.0))
                .color(Color::WHITE),
        )
        .child(
            Text::new(
                "Use the buttons to swap the preview source. The status text below is updated through media loading/success/error callbacks.",
            )
            .font_size(sp(14.0))
            .color(Color::hexa(0xBCD8ECFF)),
        )
        .child(
            Flex::new(Axis::Horizontal)
                .gap(dp(10.0))
                .child(
                    Button::new(Text::new("Preview raster"))
                        .background(Color::hexa(0x34D399FF))
                        .border_radius(dp(12.0))
                        .on_click(Command::new(ShowcaseVm::preview_embedded_raster)),
                )
                .child(
                    Button::new(Text::new("Preview SVG"))
                        .background(Color::hexa(0x7DD3FCFF))
                        .border_radius(dp(12.0))
                        .on_click(Command::new(ShowcaseVm::preview_embedded_svg)),
                ),
        )
        .child(
            Stack::new()
                .padding(Insets::all(dp(16.0)))
                .background(Color::hexa(0x08111BFF))
                .border_radius(dp(18.0))
                .child(
                    Image::new(vm.media_source.binding())
                        .size(dp(720.0), dp(300.0))
                        .fit(ContentFit::Contain)
                        .background(Color::hexa(0x102131FF))
                        .border(dp(1.0), Color::hexa(0x315977FF))
                        .border_radius(dp(18.0))
                        .on_loading(Command::new(ShowcaseVm::note_media_loading))
                        .on_success(Command::new(ShowcaseVm::note_media_ready))
                        .on_error(tgui::ValueCommand::new(ShowcaseVm::note_media_error)),
                ),
        )
        .child(
            Text::new(vm.media_status.binding())
                .padding(Insets::all(dp(12.0)))
                .background(Color::hexa(0x102131FF))
                .border_radius(dp(12.0))
                .color(Color::hexa(0xE0F2FEFF)),
        )
        .into()
}

fn media_card(
    title: &str,
    subtitle: &str,
    content: tgui::Element<ShowcaseVm>,
) -> tgui::Element<ShowcaseVm> {
    Flex::new(Axis::Vertical)
        .padding(Insets::all(dp(18.0)))
        .gap(dp(12.0))
        .background(Color::hexa(0x0F2439FF))
        .border(dp(1.0), Color::hexa(0x264761FF))
        .border_radius(dp(18.0))
        .child(Text::new(title).font_size(sp(20.0)).color(Color::WHITE))
        .child(
            Text::new(subtitle)
                .font_size(sp(14.0))
                .color(Color::hexa(0xBCD8ECFF)),
        )
        .child(content)
        .into()
}
