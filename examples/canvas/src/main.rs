use tgui::prelude::*;

fn text_style(mode: ResolvedThemeMode, size: Sp) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.typography.size = size;
    style
}

fn frame_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.background = Some(Color::hexa(0x0F172ACC).into());
    style.surface.border_radius = Some(dp(20.0).into());
    style
}

fn canvas_frame_style(mode: ResolvedThemeMode) -> CanvasStyle {
    let mut style = CanvasStyle::default_for(mode);
    style.surface.border_color = Some(Color::hexa(0x334155FF).into());
    style.surface.border_width = Some(dp(2.0).into());
    style.surface.border_radius = Some(dp(20.0).into());
    style
}

fn info_chip_style(mode: ResolvedThemeMode) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.surface.background = Some(Color::hexa(0x0F172ACC).into());
    style.surface.border_radius = Some(dp(12.0).into());
    style
}

struct CanvasVm {
    hovered: State<String>,
    activity: State<String>,
}

impl CanvasVm {
    fn items(&self) -> Vec<CanvasItem> {
        let logo = MediaSource::bytes(include_bytes!("../../../docs/images/tgui_logo.png"));
        let cutout = PathBuilder::new()
            .rect(0.0, 0.0, 140.0, 140.0)
            .difference(&PathBuilder::new().circle(70.0, 70.0, 26.0))
            .expect("boolean difference should work");

        vec![
            CanvasItem::Path(
                CanvasPath::new(
                    1_u64,
                    PathBuilder::new().rounded_rect(24.0, 24.0, 792.0, 500.0, dp(28.0)),
                )
                .fill(CanvasLinearGradient::new(
                    Point::new(24.0, 24.0),
                    Point::new(816.0, 524.0),
                    vec![
                        CanvasGradientStop::new(0.0, Color::hexa(0x0F172AFF)),
                        CanvasGradientStop::new(0.5, Color::hexa(0x111827FF)),
                        CanvasGradientStop::new(1.0, Color::hexa(0x1E293BFF)),
                    ],
                ))
                .stroke(
                    CanvasStroke::new(dp(2.0), Color::hexa(0x334155FF))
                        .line_join(CanvasStrokeJoin::Round),
                )
                .shadow(CanvasShadow::new(
                    Color::hexa(0x02061799),
                    Point::new(0.0, 14.0),
                    dp(18.0),
                )),
            ),
            CanvasItem::Text(
                CanvasText::new(2_u64, Rect::new(56.0, 48.0, 520.0, 40.0), "Canvas Scene API")
                    .text_style(CanvasTextStyle {
                        color: Color::WHITE,
                        font_size: sp(28.0),
                        line_height: Some(sp(30.0)),
                        ..Default::default()
                    }),
            ),
            CanvasItem::Text(
                CanvasText::new(
                    3_u64,
                    Rect::new(56.0, 92.0, 580.0, 64.0),
                    "Path helpers, richer stroke options, text/image items, groups, clipping, and item-level mouse events all live in the same retained scene.",
                )
                .text_style(CanvasTextStyle {
                    color: Color::hexa(0xBFDBFEFF),
                    font_size: sp(15.0),
                    line_height: Some(sp(22.0)),
                    ..Default::default()
                }),
            ),
            CanvasItem::Group(
                CanvasGroup::new(
                    10_u64,
                    vec![
                        CanvasItem::Image(
                            CanvasImage::new(11_u64, Rect::new(0.0, 0.0, 180.0, 180.0), logo)
                                .fit(ContentFit::Contain)
                                .corner_radius(dp(20.0)),
                        ),
                        CanvasItem::Path(
                            CanvasPath::new(
                                12_u64,
                                PathBuilder::new().rounded_rect(0.0, 0.0, 180.0, 180.0, dp(20.0)),
                            )
                            .stroke(
                                CanvasStroke::new(dp(3.0), Color::hexa(0x38BDF8FF))
                                    .line_join(CanvasStrokeJoin::Round),
                            ),
                        ),
                    ],
                )
                .transform(CanvasTransform2D::translate(dp(70.0), dp(190.0))),
            ),
            CanvasItem::Clip(
                CanvasClip::new(
                    20_u64,
                    CanvasClipShape::RoundedRect {
                        rect: Rect::new(300.0, 190.0, 240.0, 180.0),
                        radius: dp(24.0),
                    },
                    vec![
                        CanvasItem::Path(
                            CanvasPath::new(
                                21_u64,
                                PathBuilder::new().ellipse(420.0, 280.0, 170.0, 110.0),
                            )
                            .fill(CanvasRadialGradient::new(
                                Point::new(420.0, 280.0),
                                dp(180.0),
                                vec![
                                    CanvasGradientStop::new(0.0, Color::hexa(0xFDE68AFF)),
                                    CanvasGradientStop::new(0.45, Color::hexa(0xF97316FF)),
                                    CanvasGradientStop::new(1.0, Color::hexa(0x7C2D12FF)),
                                ],
                            )),
                        ),
                        CanvasItem::Path(
                            CanvasPath::new(22_u64, cutout)
                                .fill(Color::hexa(0xE2E8F0FF))
                                .transform(CanvasTransform2D::translate(dp(350.0), dp(210.0))),
                        ),
                        CanvasItem::Text(
                            CanvasText::new(
                                23_u64,
                                Rect::new(328.0, 396.0, 180.0, 36.0),
                                "Clip + nested items",
                            )
                            .text_style(CanvasTextStyle {
                                color: Color::hexa(0xFFF7EDFF),
                                font_size: sp(16.0),
                                ..Default::default()
                            }),
                        ),
                    ],
                ),
            ),
            CanvasItem::Path(
                CanvasPath::new(
                    30_u64,
                    PathBuilder::new()
                        .move_to(620.0, 220.0)
                        .arc(620.0, 220.0, 74.0, 0.0, std::f32::consts::PI * 1.7)
                        .close(),
                )
                .fill(Color::hexa(0x0EA5E9CC))
                .stroke(
                    CanvasStroke::new(dp(12.0), Color::hexa(0xE0F2FEFF))
                        .line_cap(CanvasStrokeCap::Round)
                        .line_join(CanvasStrokeJoin::Round)
                        .dash([dp(18.0), dp(10.0)]),
                )
                .transform(CanvasTransform2D::rotate(0.25).then(CanvasTransform2D::translate(
                    dp(72.0),
                    dp(58.0),
                ))),
            ),
            CanvasItem::Text(
                CanvasText::new(
                    31_u64,
                    Rect::new(588.0, 332.0, 170.0, 92.0),
                    "Try hovering, clicking, wheel scrolling, and dragging across the surface to see item payloads update below.",
                )
                .text_style(CanvasTextStyle {
                    color: Color::hexa(0xDBEAFEFF),
                    font_size: sp(15.0),
                    line_height: Some(sp(22.0)),
                    ..Default::default()
                }),
            ),
        ]
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
        self.activity
            .set(format!("click item={} button={:?}", event.item_id.get(), event.button));
    }

    fn on_wheel(&mut self, event: CanvasWheelEvent) {
        self.activity.set(format!(
            "wheel item={} delta=({:.0},{:.0})",
            event.item_id.get(),
            event.delta.x,
            event.delta.y
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
            hovered: ctx.state("Move across a canvas item".to_string()),
            activity: ctx.state("Click, wheel, or drag inside the canvas".to_string()),
        }
    }

    fn view(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .gap(dp(16.0))
            .child(
                Text::new("Canvas scene upgrade example")
                    .style(|mode| text_style(mode, sp(28.0)))
            )
            .child(
                Text::new(
                    "The surface below mixes PathBuilder helpers, text/image items, grouping, clipping, transforms, and richer item events.",
                )
                .style(|mode| text_style(mode, sp(15.0)))
            )
            .child(
                Stack::new()
                    .padding(Insets::all(dp(16.0)))
                    .style(frame_style)
                    .overflow_x(Overflow::Scroll)
                    .overflow_y(Overflow::Scroll)
                    .child(
                        Canvas::new(self.items())
                            .size(dp(840.0), dp(560.0))
                            .style(canvas_frame_style)
                            .on_item_mouse_move(ValueCommand::new(Self::on_hover))
                            .on_item_click(ValueCommand::new(Self::on_click))
                            .on_item_wheel(ValueCommand::new(Self::on_wheel))
                            .on_item_drag(ValueCommand::new(Self::on_drag)),
                    ),
            )
            .child(
                Flex::new(Axis::Vertical)
                    .gap(dp(8.0))
                    .child(
                        Text::new(self.hovered.signal())
                            .padding(Insets::all(dp(12.0)))
                            .style(info_chip_style)
                    )
                    .child(
                        Text::new(self.activity.signal())
                            .padding(Insets::all(dp(12.0)))
                            .style(info_chip_style)
                    ),
            )
            .into()
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .theme_mode(ThemeMode::Light)
        .title("tgui Canvas Scene")
        .window_size(dp(1180.0), dp(940.0))
        .with_view_model(CanvasVm::new)
        .root_view(CanvasVm::view)
        .run()
}
