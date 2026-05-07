use tgui::prelude::*;

struct BackgroundEffectsVm;

fn text_style(mode: ResolvedThemeMode, size: Sp, color: Color) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.typography.size = size;
    style.color = color.into();
    style
}

fn surface_style(
    mode: ResolvedThemeMode,
    radius: Dp,
    background: Option<Color>,
    brush: Option<BackgroundBrush>,
    blur: Dp,
    border: Option<(Dp, Color)>,
    background_image: Option<BackgroundImage>,
) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.background = background.map(Into::into);
    style.surface.background_brush = brush.map(Into::into);
    style.surface.background_blur = blur.into();
    style.surface.background_image = background_image.map(Into::into);
    style.surface.border_radius = Some(radius.into());
    if let Some((width, color)) = border {
        style.surface.border_width = Some(width.into());
        style.surface.border_color = Some(color.into());
    }
    style
}

impl ViewModel for BackgroundEffectsVm {

    fn new(_: &ViewModelContext) -> Self {
        Self
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .style(|mode| {
                surface_style(
                    mode,
                    dp(0.0),
                    None,
                    None,
                    Dp::ZERO,
                    None,
                    Some(
                        BackgroundImage::from_bytes(include_bytes!("../assets/juequling_shushu.jpg"))
                            .fit(ContentFit::Cover),
                    ),
                )
            })
            .child(background_pattern())
            .child(
                Flex::new(Axis::Vertical)
                    .size(pct(100.0), pct(100.0))
                    .padding(Insets::all(dp(28.0)))
                    .gap(dp(24.0))
                    .child(hero_card())
                    .child(gallery_row()),
            )
            .into()
    }
    
}

fn background_pattern() -> Element<BackgroundEffectsVm> {
    Flex::new(Axis::Vertical)
        .size(pct(100.0), pct(100.0))
        .padding(Insets::all(dp(22.0)))
        .gap(dp(18.0))
        .child(
            Flex::new(Axis::Horizontal)
                .height(dp(180.0))
                .gap(dp(18.0))
                .child(color_band(0x1D4ED8FF, 0x38BDF8FF))
                .child(color_band(0xF97316FF, 0xFB7185FF))
                .child(color_band(0x10B981FF, 0x22C55EFF)),
        )
        .child(
            Flex::new(Axis::Horizontal)
                .grow(1.0)
                .gap(dp(18.0))
                .child(color_band(0x312E81FF, 0x6D28D9FF))
                .child(color_band(0x0F766EFF, 0x14B8A6FF)),
        )
        .into()
}

fn color_band(start: u32, end: u32) -> Element<BackgroundEffectsVm> {
    Stack::new()
        .grow(1.0)
        .style(move |mode| {
            surface_style(
                mode,
                dp(28.0),
                None,
                Some(
                    BackgroundLinearGradient::new(
                        Point::new(dp(0.0), dp(0.0)),
                        Point::new(dp(260.0), dp(200.0)),
                        vec![
                            BackgroundGradientStop::new(0.0, Color::hexa(start)),
                            BackgroundGradientStop::new(1.0, Color::hexa(end)),
                        ],
                    )
                    .into(),
                ),
                Dp::ZERO,
                None,
                None,
            )
        })
        .into()
}

fn hero_card() -> Element<BackgroundEffectsVm> {
    Flex::new(Axis::Vertical)
        .padding(Insets::all(dp(24.0)))
        .gap(dp(12.0))
        .style(|mode| {
            surface_style(
                mode,
                dp(26.0),
                None,
                Some(
                    BackgroundLinearGradient::new(
                        Point::new(dp(0.0), dp(0.0)),
                        Point::new(dp(720.0), dp(0.0)),
                        vec![
                            BackgroundGradientStop::new(0.0, Color::hexa(0xFFFFFF1A)),
                            BackgroundGradientStop::new(1.0, Color::hexa(0xFFFFFF08)),
                        ],
                    )
                    .into(),
                ),
                dp(22.0),
                Some((dp(1.0), Color::hexa(0xFFFFFF44))),
                None,
            )
        })
        .child(
            Text::new("Background Effects Gallery")
                .style(|mode| text_style(mode, sp(30.0), Color::WHITE)),
        )
        .child(
            Text::new(
                "Linear gradients, radial gradients, layered glass cards, and backdrop blur on a shared background.",
            )
            .style(|mode| text_style(mode, sp(15.0), Color::hexa(0xE2E8F0FF))),
        )
        .into()
}

fn gallery_row() -> Element<BackgroundEffectsVm> {
    Flex::new(Axis::Horizontal)
        .gap(dp(20.0))
        .child(gallery_column("Linear Gradient", true, linear_gallery()))
        .child(gallery_column("Radial Gradient", true, radial_gallery()))
        .child(gallery_column("Glass Blur", false, blur_gallery()))
        .into()
}

fn gallery_column(
    title: &str,
    show_background_blur: bool,
    content: Element<BackgroundEffectsVm>,
) -> Element<BackgroundEffectsVm> {
    Flex::new(Axis::Vertical)
        .grow(1.0)
        .padding(Insets::all(dp(18.0)))
        .gap(dp(16.0))
        .style(move |mode| {
            surface_style(
                mode,
                dp(24.0),
                None,
                Some(
                    BackgroundLinearGradient::new(
                        Point::new(dp(0.0), dp(0.0)),
                        Point::new(dp(0.0), dp(480.0)),
                        vec![
                            BackgroundGradientStop::new(0.0, Color::hexa(0x0F172A88)),
                            BackgroundGradientStop::new(1.0, Color::hexa(0x11182766)),
                        ],
                    )
                    .into(),
                ),
                if show_background_blur { dp(12.0) } else { Dp::ZERO },
                Some((dp(1.0), Color::hexa(0xFFFFFF2E))),
                None,
            )
        })
        .child(Text::new(title).style(|mode| text_style(mode, sp(20.0), Color::WHITE)))
        .child(content)
        .into()
}

fn linear_gallery() -> Element<BackgroundEffectsVm> {
    Flex::new(Axis::Vertical)
        .gap(dp(12.0))
        .child(gradient_tile(
            BackgroundLinearGradient::new(
                Point::new(dp(0.0), dp(0.0)),
                Point::new(dp(220.0), dp(120.0)),
                vec![
                    BackgroundGradientStop::new(0.0, Color::hexa(0x38BDF8FF)),
                    BackgroundGradientStop::new(0.5, Color::hexa(0x6366F1FF)),
                    BackgroundGradientStop::new(1.0, Color::hexa(0x8B5CF6FF)),
                ],
            ),
            "Diagonal",
        ))
        .child(gradient_tile(
            BackgroundLinearGradient::new(
                Point::new(dp(0.0), dp(0.0)),
                Point::new(dp(0.0), dp(120.0)),
                vec![
                    BackgroundGradientStop::new(0.0, Color::hexa(0xFB7185FF)),
                    BackgroundGradientStop::new(1.0, Color::hexa(0xF97316FF)),
                ],
            ),
            "Vertical",
        ))
        .into()
}

fn radial_gallery() -> Element<BackgroundEffectsVm> {
    Flex::new(Axis::Vertical)
        .gap(dp(12.0))
        .child(radial_tile(
            BackgroundRadialGradient::new(
                Point::new(dp(70.0), dp(50.0)),
                dp(120.0),
                vec![
                    BackgroundGradientStop::new(0.0, Color::hexa(0xE0F2FEFF)),
                    BackgroundGradientStop::new(0.45, Color::hexa(0x38BDF8CC)),
                    BackgroundGradientStop::new(1.0, Color::hexa(0x0F172A00)),
                ],
            ),
            "Offset center",
        ))
        .child(radial_tile(
            BackgroundRadialGradient::new(
                Point::new(dp(150.0), dp(72.0)),
                dp(96.0),
                vec![
                    BackgroundGradientStop::new(0.0, Color::hexa(0xFEF3C7FF)),
                    BackgroundGradientStop::new(1.0, Color::hexa(0xF59E0B00)),
                ],
            ),
            "Warm glow",
        ))
        .into()
}

fn blur_gallery() -> Element<BackgroundEffectsVm> {
    Flex::new(Axis::Vertical)
        .gap(dp(12.0))
        .child(glass_tile("Blur 8", dp(8.0), Color::hexa(0xFFFFFF18)))
        .child(glass_tile("Blur 16", dp(16.0), Color::hexa(0xFFFFFF14)))
        .child(glass_tile("Blur 24", dp(24.0), Color::hexa(0xFFFFFF10)))
        .into()
}

fn gradient_tile(
    gradient: BackgroundLinearGradient,
    label: &str,
) -> Element<BackgroundEffectsVm> {
    Stack::new()
        .height(dp(110.0))
        .style(move |mode| {
            surface_style(
                mode,
                dp(18.0),
                None,
                Some(gradient.clone().into()),
                Dp::ZERO,
                None,
                None,
            )
        })
        .child(
            Text::new(label)
                .style(|mode| text_style(mode, sp(16.0), Color::WHITE))
                .padding(Insets::all(dp(14.0))),
        )
        .into()
}

fn radial_tile(
    gradient: BackgroundRadialGradient,
    label: &str,
) -> Element<BackgroundEffectsVm> {
    Stack::new()
        .height(dp(110.0))
        .style(move |mode| {
            surface_style(
                mode,
                dp(18.0),
                Some(Color::hexa(0x0F172AFF)),
                Some(gradient.clone().into()),
                Dp::ZERO,
                None,
                None,
            )
        })
        .child(
            Text::new(label)
                .style(|mode| text_style(mode, sp(16.0), Color::WHITE))
                .padding(Insets::all(dp(14.0))),
        )
        .into()
}

fn glass_tile(label: &str, blur: Dp, fill: Color) -> Element<BackgroundEffectsVm> {
    Stack::new()
        .height(dp(96.0))
        .style(move |mode| {
            surface_style(
                mode,
                dp(18.0),
                Some(fill),
                None,
                blur,
                Some((dp(1.0), Color::hexa(0xFFFFFF40))),
                None,
            )
        })
        .child(
            Text::new(label)
                .style(|mode| text_style(mode, sp(16.0), Color::WHITE))
                .padding(Insets::all(dp(14.0))),
        )
        .into()
}

fn main() -> Result<(), TguiError> {
    let mut theme = Theme::dark();
    theme.colors.background = Color::hexa(0x050816FF);
    let theme_set = ThemeSet::new(theme.clone(), theme);

    Application::new()
        .title("tgui background effects")
        .window_size(dp(1280.0), dp(860.0))
        .theme_set(theme_set)
        .theme_mode(ThemeMode::Dark)
        .with_view_model(BackgroundEffectsVm::new)
        .root_view(BackgroundEffectsVm::view)
        .run()
}
