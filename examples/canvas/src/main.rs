use tgui::prelude::*;

const CARD_CANVAS_WIDTH: f32 = 520.0;
const CARD_CANVAS_HEIGHT: f32 = 240.0;
const CARD_PANEL_HEIGHT: f32 = 362.0;
const GRID_GAP: f32 = 18.0;
const GRID_HEIGHT: f32 = CARD_PANEL_HEIGHT * 4.0 + GRID_GAP * 3.0;

fn text_style(mode: ResolvedThemeMode, size: Sp) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.typography.size = size;
    style
}

fn muted_text_style(mode: ResolvedThemeMode, size: Sp) -> TextWidgetStyle {
    let mut style = text_style(mode, size);
    style.color = Color::hexa(0x475569FF).into();
    style
}

fn hero_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.background = Some(Color::hexa(0xE2E8F0FF).into());
    style.surface.border_radius = Some(dp(24.0).into());
    style
}

fn card_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.background = Some(Color::hexa(0xF8FAFCFF).into());
    style.surface.border_color = Some(Color::hexa(0xCBD5E1FF).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(22.0).into());
    style
}

fn canvas_frame_style(mode: ResolvedThemeMode) -> CanvasStyle {
    let mut style = CanvasStyle::default_for(mode);
    style.surface.background = Some(Color::hexa(0x0F172AFF).into());
    style.surface.border_color = Some(Color::hexa(0x334155FF).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(20.0).into());
    style
}

fn info_chip_style(mode: ResolvedThemeMode) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.surface.background = Some(Color::hexa(0xE2E8F0FF).into());
    style.surface.border_radius = Some(dp(14.0).into());
    style.color = Color::hexa(0x0F172AFF).into();
    style
}

fn gallery_scroll_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.scrollbar.thumb_color = Some(Color::hexa(0x94A3B8FF));
    style.scrollbar.hover_thumb_color = Some(Color::hexa(0x64748BFF));
    style.scrollbar.active_thumb_color = Some(Color::hexa(0x475569FF));
    style.scrollbar.track_color = Some(Color::hexa(0xE2E8F0FF));
    style.scrollbar.thickness = Some(dp(10.0));
    style.scrollbar.radius = Some(dp(999.0));
    style.scrollbar.insets = Some(Insets::symmetric(dp(2.0), dp(4.0)));
    style
}

fn panel_path(id: u64, rect: Rect, fill: Color) -> CanvasItem {
    CanvasItem::Path(
        CanvasPath::new(
            id,
            PathBuilder::new().rounded_rect(
                rect.x.get(),
                rect.y.get(),
                rect.width.get(),
                rect.height.get(),
                dp(18.0),
            ),
        )
        .fill(fill),
    )
}

fn path_gallery_items() -> Vec<CanvasItem> {
    let cutout = PathBuilder::new()
        .rect(0.0, 0.0, 96.0, 96.0)
        .difference(&PathBuilder::new().circle(48.0, 48.0, 22.0))
        .expect("boolean difference should work");

    vec![
        panel_path(
            100,
            Rect::new(0.0, 0.0, CARD_CANVAS_WIDTH, CARD_CANVAS_HEIGHT),
            Color::hexa(0x0F172AFF),
        ),
        CanvasItem::Path(
            CanvasPath::new(
                101_u64,
                PathBuilder::new()
                    .move_to(54.0, 160.0)
                    .line_to(132.0, 68.0)
                    .line_to(210.0, 140.0)
                    .line_to(286.0, 56.0)
                    .line_to(366.0, 150.0)
                    .line_to(452.0, 88.0),
            )
            .stroke(
                CanvasStroke::new(dp(18.0), Color::hexa(0xE0F2FEFF))
                    .line_cap(CanvasStrokeCap::Round)
                    .line_join(CanvasStrokeJoin::Round)
                    .dash([dp(24.0), dp(14.0)]),
            )
            .shadow(CanvasShadow::new(
                Color::hexa(0x0EA5E966),
                Point::new(0.0, 10.0),
                dp(18.0),
            )),
        ),
        CanvasItem::Path(
            CanvasPath::new(
                102_u64,
                PathBuilder::new().rounded_rect(44.0, 34.0, 168.0, 84.0, dp(26.0)),
            )
            .fill(CanvasLinearGradient::new(
                Point::new(44.0, 34.0),
                Point::new(212.0, 118.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0x38BDF8FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                ],
            ))
            .opacity(0.92),
        ),
        CanvasItem::Path(
            CanvasPath::new(103_u64, cutout)
                .fill(Color::hexa(0xF8FAFCFF))
                .transform(CanvasTransform2D::translate(dp(340.0), dp(74.0)))
                .opacity(0.85),
        ),
        CanvasItem::Text(
            CanvasText::new(
                104_u64,
                Rect::new(48.0, 188.0, 420.0, 24.0),
                "Gradients, dash strokes, shadows, and boolean paths",
            )
            .text_style(CanvasTextStyle {
                color: Color::hexa(0xBFDBFEFF),
                font_size: sp(15.0),
                ..Default::default()
            }),
        ),
    ]
}

fn paragraph_gallery_items() -> Vec<CanvasItem> {
    vec![
        panel_path(
            200_u64,
            Rect::new(0.0, 0.0, CARD_CANVAS_WIDTH, CARD_CANVAS_HEIGHT),
            Color::hexa(0xF8FAFCFF),
        ),
        panel_path(
            201_u64,
            Rect::new(24.0, 24.0, 226.0, 88.0),
            Color::hexa(0xE2E8F0FF),
        ),
        panel_path(
            202_u64,
            Rect::new(270.0, 24.0, 226.0, 88.0),
            Color::hexa(0xDBEAFEFF),
        ),
        panel_path(
            203_u64,
            Rect::new(24.0, 132.0, 472.0, 84.0),
            Color::hexa(0xE0F2FEFF),
        ),
        CanvasItem::Text(
            CanvasText::new(
                204_u64,
                Rect::new(36.0, 36.0, 202.0, 64.0),
                "Word wrap keeps phrases together and centers the block.",
            )
            .text_style(CanvasTextStyle {
                color: Color::hexa(0x0F172AFF),
                font_size: sp(15.0),
                line_height: Some(sp(20.0)),
                ..Default::default()
            })
            .paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Word,
                horizontal_align: CanvasTextHorizontalAlign::Center,
                vertical_align: CanvasTextVerticalAlign::Center,
                ..Default::default()
            }),
        ),
        CanvasItem::Text(
            CanvasText::new(
                205_u64,
                Rect::new(282.0, 36.0, 202.0, 64.0),
                "Glyph wrap can break superlongtokenswithoutspaces cleanly.",
            )
            .text_style(CanvasTextStyle {
                color: Color::hexa(0x0F172AFF),
                font_size: sp(15.0),
                line_height: Some(sp(20.0)),
                ..Default::default()
            })
            .paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Glyph,
                horizontal_align: CanvasTextHorizontalAlign::Start,
                vertical_align: CanvasTextVerticalAlign::Center,
                ..Default::default()
            }),
        ),
        CanvasItem::Text(
            CanvasText::new(
                206_u64,
                Rect::new(40.0, 144.0, 440.0, 60.0),
                "No wrap stays on one line, clips overflow, and can align to the trailing edge.",
            )
            .text_style(CanvasTextStyle {
                color: Color::hexa(0x0F172AFF),
                font_size: sp(16.0),
                line_height: Some(sp(21.0)),
                ..Default::default()
            })
            .paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::None,
                horizontal_align: CanvasTextHorizontalAlign::End,
                vertical_align: CanvasTextVerticalAlign::Center,
                ..Default::default()
            }),
        ),
    ]
}

fn transform_gallery_items() -> Vec<CanvasItem> {
    let logo = MediaSource::bytes(include_bytes!("../../../docs/images/tgui_logo.png"));
    let shear = CanvasTransform2D::from_matrix([1.0, 0.0, 0.35, 1.0, 0.0, 0.0]);

    vec![
        panel_path(300_u64, Rect::new(0.0, 0.0, CARD_CANVAS_WIDTH, CARD_CANVAS_HEIGHT), Color::hexa(0x111827FF)),
        CanvasItem::Path(
            CanvasPath::new(
                301_u64,
                PathBuilder::new().rounded_rect(48.0, 40.0, 150.0, 150.0, dp(28.0)),
            )
            .stroke(CanvasStroke::new(dp(2.0), Color::hexa(0x334155FF))),
        ),
        CanvasItem::Image(
            CanvasImage::new(302_u64, Rect::new(66.0, 58.0, 114.0, 114.0), logo)
                .fit(ContentFit::Contain)
                .corner_radius(dp(18.0))
                .transform(
                    CanvasTransform2D::rotate(-0.24)
                        .then(CanvasTransform2D::translate(dp(-18.0), dp(32.0))),
                ),
        ),
        CanvasItem::Text(
            CanvasText::new(303_u64, Rect::new(250.0, 44.0, 200.0, 86.0), "Rotated image")
                .text_style(CanvasTextStyle {
                    color: Color::hexa(0xE2E8F0FF),
                    font_size: sp(24.0),
                    font_weight: FontWeight::Bold,
                    ..Default::default()
                })
                .transform(
                    shear.then(CanvasTransform2D::translate(dp(22.0), dp(-8.0))),
                )
                .opacity(0.94),
        ),
        CanvasItem::Text(
            CanvasText::new(
                304_u64,
                Rect::new(232.0, 120.0, 238.0, 74.0),
                "Text now follows arbitrary affine transforms instead of snapping back to axis-aligned rectangles.",
            )
            .text_style(CanvasTextStyle {
                color: Color::hexa(0xBFDBFEFF),
                font_size: sp(15.0),
                line_height: Some(sp(21.0)),
                ..Default::default()
            })
            .paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Word,
                ..Default::default()
            }),
        ),
    ]
}

fn opacity_gallery_items() -> Vec<CanvasItem> {
    let logo = MediaSource::bytes(include_bytes!("../../../docs/images/tgui_logo.png"));

    vec![
        panel_path(
            400_u64,
            Rect::new(0.0, 0.0, CARD_CANVAS_WIDTH, CARD_CANVAS_HEIGHT),
            Color::hexa(0x0F172AFF),
        ),
        CanvasItem::Path(
            CanvasPath::new(401_u64, PathBuilder::new().circle(128.0, 106.0, 74.0))
                .fill(Color::hexa(0x22C55EFF))
                .opacity(0.28),
        ),
        CanvasItem::Image(
            CanvasImage::new(402_u64, Rect::new(186.0, 48.0, 148.0, 148.0), logo)
                .fit(ContentFit::Contain)
                .corner_radius(dp(18.0))
                .opacity(0.58),
        ),
        CanvasItem::Text(
            CanvasText::new(403_u64, Rect::new(314.0, 74.0, 138.0, 82.0), "Text opacity")
                .text_style(CanvasTextStyle {
                    color: Color::hexa(0xF8FAFCFF),
                    font_size: sp(28.0),
                    font_weight: FontWeight::Bold,
                    ..Default::default()
                })
                .transform(
                    CanvasTransform2D::rotate(0.18)
                        .then(CanvasTransform2D::translate(dp(44.0), dp(-30.0))),
                )
                .opacity(0.52),
        ),
        CanvasItem::Text(
            CanvasText::new(
                404_u64,
                Rect::new(42.0, 188.0, 430.0, 24.0),
                "Path, image, and text opacity all blend with parent opacity now.",
            )
            .text_style(CanvasTextStyle {
                color: Color::hexa(0xCBD5E1FF),
                font_size: sp(15.0),
                ..Default::default()
            }),
        ),
    ]
}

fn clip_gallery_items() -> Vec<CanvasItem> {
    let path_clip = PathBuilder::new()
        .move_to(388.0, 40.0)
        .line_to(470.0, 74.0)
        .line_to(454.0, 174.0)
        .line_to(360.0, 196.0)
        .line_to(306.0, 124.0)
        .close();

    vec![
        panel_path(
            500_u64,
            Rect::new(0.0, 0.0, CARD_CANVAS_WIDTH, CARD_CANVAS_HEIGHT),
            Color::hexa(0xF8FAFCFF),
        ),
        CanvasItem::Clip(CanvasClip::new(
            501_u64,
            CanvasClipShape::RoundedRect {
                rect: Rect::new(24.0, 28.0, 220.0, 180.0),
                radius: dp(30.0),
            },
            vec![
                CanvasItem::Path(
                    CanvasPath::new(
                        502_u64,
                        PathBuilder::new().ellipse(130.0, 118.0, 148.0, 106.0),
                    )
                    .fill(CanvasRadialGradient::new(
                        Point::new(120.0, 104.0),
                        dp(156.0),
                        vec![
                            CanvasGradientStop::new(0.0, Color::hexa(0xFDE68AFF)),
                            CanvasGradientStop::new(0.5, Color::hexa(0xFB7185FF)),
                            CanvasGradientStop::new(1.0, Color::hexa(0x7C3AEDFF)),
                        ],
                    )),
                ),
                CanvasItem::Text(
                    CanvasText::new(503_u64, Rect::new(54.0, 148.0, 160.0, 40.0), "Rounded clip")
                        .text_style(CanvasTextStyle {
                            color: Color::WHITE,
                            font_size: sp(20.0),
                            font_weight: FontWeight::Bold,
                            ..Default::default()
                        }),
                ),
            ],
        )),
        CanvasItem::Clip(CanvasClip::new(
            504_u64,
            CanvasClipShape::Path(path_clip),
            vec![
                CanvasItem::Path(
                    CanvasPath::new(
                        505_u64,
                        PathBuilder::new().rounded_rect(276.0, 30.0, 210.0, 176.0, dp(28.0)),
                    )
                    .fill(CanvasLinearGradient::new(
                        Point::new(276.0, 30.0),
                        Point::new(486.0, 206.0),
                        vec![
                            CanvasGradientStop::new(0.0, Color::hexa(0x0EA5E9FF)),
                            CanvasGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                        ],
                    )),
                ),
                CanvasItem::Path(
                    CanvasPath::new(506_u64, PathBuilder::new().circle(420.0, 116.0, 56.0))
                        .fill(Color::hexa(0xF8FAFCFF))
                        .opacity(0.9),
                ),
                CanvasItem::Text(
                    CanvasText::new(507_u64, Rect::new(328.0, 150.0, 126.0, 32.0), "Path clip")
                        .text_style(CanvasTextStyle {
                            color: Color::hexa(0x0F172AFF),
                            font_size: sp(18.0),
                            font_weight: FontWeight::Bold,
                            ..Default::default()
                        }),
                ),
            ],
        )),
    ]
}

fn mask_gallery_items() -> Vec<CanvasItem> {
    vec![
        panel_path(
            600_u64,
            Rect::new(0.0, 0.0, CARD_CANVAS_WIDTH, CARD_CANVAS_HEIGHT),
            Color::hexa(0x0F172AFF),
        ),
        CanvasItem::Mask(CanvasMask::new(
            601_u64,
            vec![
                CanvasItem::Text(
                    CanvasText::new(602_u64, Rect::new(78.0, 70.0, 364.0, 88.0), "MASK")
                        .text_style(CanvasTextStyle {
                            color: Color::WHITE,
                            font_size: sp(78.0),
                            font_weight: FontWeight::Bold,
                            ..Default::default()
                        }),
                ),
                CanvasItem::Path(
                    CanvasPath::new(603_u64, PathBuilder::new().circle(420.0, 86.0, 28.0))
                        .fill(Color::WHITE),
                ),
            ],
            vec![
                CanvasItem::Path(
                    CanvasPath::new(
                        604_u64,
                        PathBuilder::new().rounded_rect(52.0, 54.0, 416.0, 122.0, dp(28.0)),
                    )
                    .fill(CanvasLinearGradient::new(
                        Point::new(52.0, 54.0),
                        Point::new(468.0, 176.0),
                        vec![
                            CanvasGradientStop::new(0.0, Color::hexa(0x14B8A6FF)),
                            CanvasGradientStop::new(0.5, Color::hexa(0x22D3EEFF)),
                            CanvasGradientStop::new(1.0, Color::hexa(0x818CF8FF)),
                        ],
                    )),
                ),
                CanvasItem::Path(
                    CanvasPath::new(
                        605_u64,
                        PathBuilder::new().ellipse(258.0, 114.0, 190.0, 64.0),
                    )
                    .fill(Color::hexa(0xFFFFFF44)),
                ),
            ],
        )),
        CanvasItem::Text(
            CanvasText::new(
                606_u64,
                Rect::new(64.0, 188.0, 390.0, 24.0),
                "Mask alpha comes from its own subtree, then gates the content subtree.",
            )
            .text_style(CanvasTextStyle {
                color: Color::hexa(0xCBD5E1FF),
                font_size: sp(15.0),
                ..Default::default()
            }),
        ),
    ]
}

fn layer_blend_gallery_items() -> Vec<CanvasItem> {
    vec![
        panel_path(
            700_u64,
            Rect::new(0.0, 0.0, CARD_CANVAS_WIDTH, CARD_CANVAS_HEIGHT),
            Color::hexa(0x020617FF),
        ),
        CanvasItem::Path(
            CanvasPath::new(701_u64, PathBuilder::new().circle(176.0, 112.0, 74.0))
                .fill(Color::hexa(0x14B8A6FF)),
        ),
        CanvasItem::Path(
            CanvasPath::new(702_u64, PathBuilder::new().circle(240.0, 112.0, 74.0))
                .fill(Color::hexa(0xF43F5EFF))
                .opacity(0.92),
        ),
        CanvasItem::Layer(
            CanvasLayer::new(
                703_u64,
                vec![
                    CanvasItem::Path(
                        CanvasPath::new(704_u64, PathBuilder::new().circle(206.0, 88.0, 62.0))
                            .fill(Color::hexa(0x38BDF8FF)),
                    ),
                    CanvasItem::Path(
                        CanvasPath::new(705_u64, PathBuilder::new().circle(252.0, 136.0, 62.0))
                            .fill(Color::hexa(0xFDE047FF)),
                    ),
                ],
            )
            .blend_mode(CanvasBlendMode::Screen)
            .opacity(0.86),
        ),
        CanvasItem::Group(
            CanvasGroup::new(
                706_u64,
                vec![
                    panel_path(
                        707_u64,
                        Rect::new(330.0, 48.0, 132.0, 112.0),
                        Color::hexa(0x111827FF),
                    ),
                    CanvasItem::Text(
                        CanvasText::new(
                            708_u64,
                            Rect::new(346.0, 72.0, 100.0, 48.0),
                            "Group opacity",
                        )
                        .text_style(CanvasTextStyle {
                            color: Color::WHITE,
                            font_size: sp(22.0),
                            font_weight: FontWeight::Bold,
                            ..Default::default()
                        })
                        .paragraph_style(CanvasParagraphStyle {
                            horizontal_align: CanvasTextHorizontalAlign::Center,
                            vertical_align: CanvasTextVerticalAlign::Center,
                            wrap: CanvasTextWrap::Word,
                            ..Default::default()
                        }),
                    ),
                ],
            )
            .opacity(0.64)
            .transform(
                CanvasTransform2D::rotate(-0.16)
                    .then(CanvasTransform2D::translate(dp(-10.0), dp(66.0))),
            ),
        ),
        CanvasItem::Text(
            CanvasText::new(
                709_u64,
                Rect::new(42.0, 190.0, 432.0, 24.0),
                "Layers isolate blend math; group opacity no longer leaks into child primitives.",
            )
            .text_style(CanvasTextStyle {
                color: Color::hexa(0xBFDBFEFF),
                font_size: sp(15.0),
                ..Default::default()
            }),
        ),
    ]
}

fn interaction_gallery_items() -> Vec<CanvasItem> {
    vec![
        panel_path(800_u64, Rect::new(0.0, 0.0, CARD_CANVAS_WIDTH, CARD_CANVAS_HEIGHT), Color::hexa(0xF8FAFCFF)),
        CanvasItem::Path(
            CanvasPath::new(
                801_u64,
                PathBuilder::new().rounded_rect(42.0, 54.0, 128.0, 72.0, dp(18.0)),
            )
            .fill(Color::hexa(0x2563EBFF)),
        ),
        CanvasItem::Text(
            CanvasText::new(802_u64, Rect::new(58.0, 76.0, 96.0, 28.0), "Click me")
                .text_style(CanvasTextStyle {
                    color: Color::WHITE,
                    font_size: sp(22.0),
                    font_weight: FontWeight::Bold,
                    ..Default::default()
                })
                .cursor(CursorStyle::Pointer),
        ),
        CanvasItem::Path(
            CanvasPath::new(
                803_u64,
                PathBuilder::new().rounded_rect(212.0, 52.0, 108.0, 76.0, dp(18.0)),
            )
            .fill(Color::hexa(0x0F766EFF))
            .cursor(CursorStyle::Grab),
        ),
        CanvasItem::Text(
            CanvasText::new(804_u64, Rect::new(226.0, 76.0, 80.0, 28.0), "Drag")
                .text_style(CanvasTextStyle {
                    color: Color::WHITE,
                    font_size: sp(22.0),
                    font_weight: FontWeight::Bold,
                    ..Default::default()
                })
                .cursor(CursorStyle::Grab),
        ),
        CanvasItem::Path(
            CanvasPath::new(805_u64, PathBuilder::new().circle(410.0, 92.0, 42.0))
                .fill(Color::hexa(0xF59E0BFF))
                .cursor(CursorStyle::Crosshair),
        ),
        CanvasItem::Text(
            CanvasText::new(806_u64, Rect::new(378.0, 82.0, 64.0, 20.0), "Wheel")
                .text_style(CanvasTextStyle {
                    color: Color::hexa(0x0F172AFF),
                    font_size: sp(18.0),
                    font_weight: FontWeight::Bold,
                    ..Default::default()
                })
                .cursor(CursorStyle::Crosshair),
        ),
        CanvasItem::Text(
            CanvasText::new(
                807_u64,
                Rect::new(40.0, 158.0, 436.0, 42.0),
                "Hover, click, wheel, and drag any of the targets above. The live payload summary below the header updates from item-level canvas events.",
            )
            .text_style(CanvasTextStyle {
                color: Color::hexa(0x334155FF),
                font_size: sp(15.0),
                line_height: Some(sp(21.0)),
                ..Default::default()
            })
            .paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Word,
                ..Default::default()
            }),
        ),
    ]
}

struct CanvasVm {
    hovered: State<String>,
    activity: State<String>,
}

impl CanvasVm {
    fn sample_canvas(&self, items: Vec<CanvasItem>) -> Canvas<Self> {
        Canvas::new(items)
            .size(dp(CARD_CANVAS_WIDTH), dp(CARD_CANVAS_HEIGHT))
            .style(canvas_frame_style)
            .on_item_mouse_move(ValueCommand::new(Self::on_hover))
            .on_item_click(ValueCommand::new(Self::on_click))
            .on_item_drag(ValueCommand::new(Self::on_drag))
    }

    fn example_card(
        &self,
        title: &'static str,
        description: &'static str,
        items: Vec<CanvasItem>,
    ) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .height(dp(CARD_PANEL_HEIGHT))
            .padding(Insets::all(dp(18.0)))
            .gap(dp(12.0))
            .style(card_style)
            .child(Text::new(title).style(|mode| text_style(mode, sp(22.0))))
            .child(Text::new(description).style(|mode| muted_text_style(mode, sp(14.0))))
            .child(self.sample_canvas(items))
            .into()
    }

    fn on_hover(&mut self, event: CanvasMouseEvent) {
        self.hovered.set(format!(
            "hover item={} canvas=({:.0},{:.0}) scene=({:.0},{:.0}) local=({:.0},{:.0})",
            event.item_id.get(),
            event.canvas_position.x,
            event.canvas_position.y,
            event.scene_position.x,
            event.scene_position.y,
            event.local_position.x,
            event.local_position.y
        ));
    }

    fn on_click(&mut self, event: CanvasMouseEvent) {
        self.activity.set(format!(
            "click item={} button={:?}",
            event.item_id.get(),
            event.button
        ));
    }

    fn on_drag(&mut self, event: CanvasDragEvent) {
        self.activity.set(format!(
            "drag item={} delta=({:.0},{:.0})",
            event.item_id.get(),
            event.delta.x,
            event.delta.y
        ));
    }
}

impl ViewModel for CanvasVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            hovered: ctx.state("Move across any canvas cell to inspect item payloads.".to_string()),
            activity: ctx.state("Click or drag inside a cell to see the latest event.".to_string()),
        }
    }

    fn view(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .gap(dp(16.0))
            .overflow_x(Overflow::Hidden)
            .overflow_y(Overflow::Hidden)
            .child(
                Flex::new(Axis::Vertical)
                    .padding(Insets::all(dp(20.0)))
                    .gap(dp(10.0))
                    .style(hero_style)
                    .child(Text::new("Canvas API Gallery").style(|mode| text_style(mode, sp(30.0))))
                    .child(
                        Text::new(
                            "Each grid cell below isolates one Canvas capability so the public API is easier to verify visually: paths, paragraph layout, transforms, opacity, clip, mask, layer blending, and item-level events.",
                        )
                        .style(|mode| muted_text_style(mode, sp(15.0))),
                    ),
            )
            .child(
                Flex::new(Axis::Vertical)
                    .gap(dp(8.0))
                    .child(
                        Text::new(self.hovered.signal())
                            .padding(Insets::all(dp(12.0)))
                            .style(info_chip_style),
                    )
                    .child(
                        Text::new(self.activity.signal())
                            .padding(Insets::all(dp(12.0)))
                            .style(info_chip_style),
                    ),
            )
            .child(
                Stack::new()
                    .min_height(dp(0.0))
                    .grow(1.0)
                    .overflow_y(Overflow::Scroll)
                    .overflow_x(Overflow::Hidden)
                    .style(gallery_scroll_style)
                    .child(
                        Grid::columns([fr(1.0), fr(1.0)])
                            .height(dp(GRID_HEIGHT))
                            .gap(dp(GRID_GAP))
                            .width(pct(100.0))
                            .child(el![
                                self.example_card(
                                    "Paths",
                                    "Gradient fills, dash strokes, shadows, and boolean path helpers.",
                                    path_gallery_items(),
                                ),
                                self.example_card(
                                    "Paragraph Style",
                                    "Word, glyph, and no-wrap modes with horizontal and vertical alignment.",
                                    paragraph_gallery_items(),
                                ),
                                self.example_card(
                                    "Transforms",
                                    "Images and text rendered through arbitrary affine quads instead of axis-only shortcuts.",
                                    transform_gallery_items(),
                                ),
                                self.example_card(
                                    "Leaf Opacity",
                                    "Path, image, and text opacity all participate in final compositing.",
                                    opacity_gallery_items(),
                                ),
                                self.example_card(
                                    "Clip Shapes",
                                    "Rounded-rect and path clips both isolate their nested content visually.",
                                    clip_gallery_items(),
                                ),
                                self.example_card(
                                    "Mask",
                                    "Mask alpha comes from its own subtree and gates a separate content subtree.",
                                    mask_gallery_items(),
                                ),
                                self.example_card(
                                    "Layer and Blend",
                                    "Isolated layers preserve blend math and group opacity semantics.",
                                    layer_blend_gallery_items(),
                                ),
                                self.example_card(
                                    "Events",
                                    "Item-level hover, click, and drag stay active in each independent canvas cell while the page itself keeps wheel scrolling.",
                                    interaction_gallery_items(),
                                )
                            ]),
                    ),
            )
            .into()
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .msaa(MsaaMode::X4)
        .theme_mode(ThemeMode::Light)
        .title("tgui Canvas Gallery")
        .window_size(dp(1480.0), dp(1080.0))
        .with_view_model(CanvasVm::new)
        .root_view(CanvasVm::view)
        .run()
}
