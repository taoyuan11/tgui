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

fn card_scene_background(canvas: &mut CanvasRecorder, color: Color) {
    canvas
        .set_fill(color)
        .fill_round_rect(0.0, 0.0, CARD_CANVAS_WIDTH, CARD_CANVAS_HEIGHT, dp(20.0));
}

fn paths_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0x0F172AFF));
        canvas
            .set_fill(CanvasLinearGradient::new(
                Point::new(44.0, 34.0),
                Point::new(212.0, 118.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0x38BDF8FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                ],
            ))
            .fill_round_rect(44.0, 34.0, 168.0, 84.0, dp(26.0));

        canvas
            .set_stroke(
                CanvasStroke::new(dp(18.0), Color::hexa(0xE0F2FEFF))
                    .line_cap(CanvasStrokeCap::Round)
                    .line_join(CanvasStrokeJoin::Round)
                    .dash([dp(24.0), dp(14.0)]),
            )
            .set_shadow(CanvasShadow::new(
                Color::hexa(0x0EA5E966),
                Point::new(0.0, 10.0),
                dp(18.0),
            ))
            .begin_path()
            .move_to(54.0, 160.0)
            .line_to(132.0, 68.0)
            .line_to(210.0, 140.0)
            .line_to(286.0, 56.0)
            .line_to(366.0, 150.0)
            .line_to(452.0, 88.0)
            .stroke();

        canvas
            .clear_shadow()
            .set_fill(Color::hexa(0xF8FAFCFF))
            .fill_circle(388.0, 122.0, 54.0)
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0xBFDBFEFF),
                font_size: sp(15.0),
                ..Default::default()
            })
            .draw_text(
                Rect::new(48.0, 188.0, 420.0, 24.0),
                "Recorder paths cover gradients, dash strokes, shadows, and freeform drawing.",
            );
    })
}

fn text_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0xF8FAFCFF));
        canvas
            .set_fill(Color::hexa(0xE2E8F0FF))
            .fill_round_rect(24.0, 24.0, 226.0, 88.0, dp(20.0))
            .set_fill(Color::hexa(0xDBEAFEFF))
            .fill_round_rect(270.0, 24.0, 226.0, 88.0, dp(20.0))
            .set_fill(Color::hexa(0xE0F2FEFF))
            .fill_round_rect(24.0, 132.0, 472.0, 84.0, dp(20.0));

        canvas
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0x0F172AFF),
                font_size: sp(15.0),
                line_height: Some(sp(20.0)),
                ..Default::default()
            })
            .set_paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Word,
                horizontal_align: CanvasTextHorizontalAlign::Center,
                vertical_align: CanvasTextVerticalAlign::Center,
                ..Default::default()
            })
            .draw_text(
                Rect::new(36.0, 36.0, 202.0, 64.0),
                "Word wrap keeps phrases together and centers the block.",
            )
            .set_paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Glyph,
                horizontal_align: CanvasTextHorizontalAlign::Start,
                vertical_align: CanvasTextVerticalAlign::Center,
                ..Default::default()
            })
            .draw_text(
                Rect::new(282.0, 36.0, 202.0, 64.0),
                "Glyph wrap can break superlongtokenswithoutspaces cleanly.",
            )
            .set_paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Word,
                overflow: CanvasTextOverflow::Ellipsis,
                vertical_align: CanvasTextVerticalAlign::Center,
                ..Default::default()
            })
            .draw_text(
                Rect::new(40.0, 144.0, 440.0, 54.0),
                "Ellipsis now works in the public recorder API and keeps long labels readable inside fixed cards.",
            );
    })
}

fn transform_scene() -> CanvasScene {
    let logo = MediaSource::bytes(include_bytes!("../../../docs/images/tgui_logo.png"));
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0x111827FF));
        canvas
            .set_fill(Color::hexa(0x111827FF))
            .set_stroke(CanvasStroke::new(dp(2.0), Color::hexa(0x334155FF)))
            .fill_round_rect(48.0, 40.0, 150.0, 150.0, dp(28.0))
            .stroke_round_rect(48.0, 40.0, 150.0, 150.0, dp(28.0));

        canvas
            .save()
            .translate(124.0, 115.0)
            .rotate(-0.24)
            .translate(-58.0, -57.0)
            .draw_image(Rect::new(0.0, 0.0, 114.0, 114.0), logo)
            .restore();

        canvas
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0xE2E8F0FF),
                font_size: sp(22.0),
                font_weight: FontWeight::Bold,
                ..Default::default()
            })
            .draw_text(Rect::new(250.0, 44.0, 200.0, 36.0), "Transforms")
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0xCBD5E1FF),
                font_size: sp(15.0),
                line_height: Some(sp(20.0)),
                ..Default::default()
            })
            .draw_text(
                Rect::new(250.0, 92.0, 214.0, 92.0),
                "save/restore isolates transforms so rotated images and translated labels stay local to each drawing pass.",
            );
    })
}

fn clip_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0xF8FAFCFF));
        canvas
            .save()
            .rect(24.0, 28.0, 220.0, 180.0)
            .clip()
            .set_fill(CanvasRadialGradient::new(
                Point::new(120.0, 104.0),
                dp(156.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0xFDE68AFF)),
                    CanvasGradientStop::new(0.5, Color::hexa(0xFB7185FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x7C3AEDFF)),
                ],
            ))
            .fill_ellipse(130.0, 118.0, 148.0, 106.0)
            .set_text_style(CanvasTextStyle {
                color: Color::WHITE,
                font_size: sp(20.0),
                font_weight: FontWeight::Bold,
                ..Default::default()
            })
            .draw_text(Rect::new(54.0, 148.0, 160.0, 40.0), "Rect clip")
            .restore();

        canvas
            .save()
            .begin_path()
            .move_to(388.0, 40.0)
            .line_to(470.0, 74.0)
            .line_to(454.0, 174.0)
            .line_to(360.0, 196.0)
            .line_to(306.0, 124.0)
            .close_path()
            .clip()
            .set_fill(CanvasLinearGradient::new(
                Point::new(276.0, 30.0),
                Point::new(486.0, 206.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0x0EA5E9FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                ],
            ))
            .fill_round_rect(276.0, 30.0, 210.0, 176.0, dp(28.0))
            .set_fill(Color::hexa(0xF8FAFCFF))
            .fill_circle(420.0, 116.0, 56.0)
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0x0F172AFF),
                font_size: sp(18.0),
                font_weight: FontWeight::Bold,
                ..Default::default()
            })
            .draw_text(Rect::new(328.0, 150.0, 126.0, 32.0), "Path clip")
            .restore();
    })
}

fn blend_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0x020617FF));
        canvas
            .set_fill(Color::hexa(0x14B8A6FF))
            .fill_circle(176.0, 112.0, 74.0)
            .set_fill(Color::hexa(0xF43F5EFF))
            .set_opacity(0.92)
            .fill_circle(240.0, 112.0, 74.0)
            .set_blend_mode(CanvasBlendMode::Screen)
            .set_fill(Color::hexa(0x38BDF8FF))
            .fill_circle(206.0, 88.0, 62.0)
            .set_fill(Color::hexa(0xFDE047FF))
            .fill_circle(252.0, 136.0, 62.0)
            .set_blend_mode(CanvasBlendMode::Normal)
            .set_opacity(1.0)
            .save()
            .translate(348.0, 106.0)
            .rotate(-0.16)
            .translate(-66.0, -56.0)
            .set_fill(Color::hexa(0x111827FF))
            .set_opacity(0.64)
            .fill_round_rect(0.0, 0.0, 132.0, 112.0, dp(20.0))
            .set_text_style(CanvasTextStyle {
                color: Color::WHITE,
                font_size: sp(22.0),
                font_weight: FontWeight::Bold,
                ..Default::default()
            })
            .set_paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Word,
                horizontal_align: CanvasTextHorizontalAlign::Center,
                vertical_align: CanvasTextVerticalAlign::Center,
                ..Default::default()
            })
            .draw_text(Rect::new(16.0, 24.0, 100.0, 48.0), "Blend + alpha")
            .restore();
    })
}

fn recorder_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0xF8FAFCFF));
        canvas
            .set_fill(CanvasLinearGradient::new(
                Point::new(24.0, 28.0),
                Point::new(220.0, 148.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0x0EA5E9FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                ],
            ))
            .fill_round_rect(24.0, 28.0, 180.0, 118.0, dp(26.0))
            .save()
            .translate(288.0, 34.0)
            .rotate(-0.14)
            .set_fill(Color::hexa(0x0F172AFF))
            .fill_round_rect(0.0, 0.0, 168.0, 110.0, dp(24.0))
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0xE2E8F0FF),
                font_size: sp(18.0),
                line_height: Some(sp(24.0)),
                ..Default::default()
            })
            .draw_text(Rect::new(20.0, 18.0, 128.0, 70.0), "save/restore keeps transform state local")
            .restore()
            .save()
            .rect(236.0, 32.0, 238.0, 86.0)
            .clip()
            .set_fill(Color::hexa(0xCBD5E1FF))
            .fill_rect(246.0, 42.0, 92.0, 92.0)
            .set_fill(Color::hexa(0x38BDF8AA))
            .fill_circle(402.0, 82.0, 56.0)
            .restore()
            .next_item_id(950_u64)
            .set_fill(Color::hexa(0x0F172AFF))
            .fill_circle(92.0, 184.0, 28.0)
            .set_stroke(
                CanvasStroke::new(dp(10.0), Color::hexa(0xF97316FF))
                    .line_cap(CanvasStrokeCap::Round)
                    .line_join(CanvasStrokeJoin::Round),
            )
            .draw_line(148.0, 186.0, 244.0, 186.0)
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0x0F172AFF),
                font_size: sp(16.0),
                line_height: Some(sp(22.0)),
                ..Default::default()
            })
            .set_paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Word,
                overflow: CanvasTextOverflow::Ellipsis,
                ..Default::default()
            })
            .draw_text(
                Rect::new(270.0, 154.0, 210.0, 54.0),
                "Recorder API emits scene data, preserves IDs, and ellipsizes long labels cleanly.",
            );
    })
}

fn interaction_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0xF8FAFCFF));
        canvas
            .next_item_id(801_u64)
            .set_fill(Color::hexa(0x38BDF8FF))
            .fill_round_rect(42.0, 54.0, 128.0, 72.0, dp(18.0))
            .set_text_style(CanvasTextStyle {
                color: Color::WHITE,
                font_size: sp(18.0),
                font_weight: FontWeight::Bold,
                ..Default::default()
            })
            .draw_text(Rect::new(70.0, 76.0, 76.0, 28.0), "Click")
            .next_item_id(803_u64)
            .set_fill(Color::hexa(0x22C55EFF))
            .fill_round_rect(212.0, 52.0, 108.0, 76.0, dp(18.0))
            .draw_text(Rect::new(234.0, 76.0, 70.0, 28.0), "Drag")
            .next_item_id(805_u64)
            .set_fill(Color::hexa(0xF97316FF))
            .fill_circle(410.0, 92.0, 42.0)
            .draw_text(Rect::new(382.0, 82.0, 58.0, 22.0), "Hover")
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0x334155FF),
                font_size: sp(15.0),
                line_height: Some(sp(21.0)),
                ..Default::default()
            })
            .set_paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Word,
                ..Default::default()
            })
            .draw_text(
                Rect::new(40.0, 158.0, 436.0, 42.0),
                "Hover, click, wheel, and drag the targets above. The live summary below updates from item-level recorder IDs.",
            );
    })
}

struct CanvasVm {
    hovered: State<String>,
    activity: State<String>,
}

impl CanvasVm {
    fn sample_canvas(&self, scene: CanvasScene) -> Canvas<Self> {
        Canvas::new(scene)
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
        scene: CanvasScene,
    ) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .height(dp(CARD_PANEL_HEIGHT))
            .padding(Insets::all(dp(18.0)))
            .gap(dp(12.0))
            .style(card_style)
            .child(Text::new(title).style(|mode| text_style(mode, sp(22.0))))
            .child(Text::new(description).style(|mode| muted_text_style(mode, sp(14.0))))
            .child(self.sample_canvas(scene))
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
                    .child(Text::new("Canvas Recorder Gallery").style(|mode| text_style(mode, sp(30.0))))
                    .child(
                        Text::new(
                            "This gallery uses only the public recorder-style Canvas API: path commands, transforms, clipping, blending, text overflow, image drawing, and stable item IDs.",
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
                                    "Gradients, dash strokes, shadows, and freeform path recording.",
                                    paths_scene(),
                                ),
                                self.example_card(
                                    "Text",
                                    "Wrap, alignment, and ellipsis now all live in the recorder API.",
                                    text_scene(),
                                ),
                                self.example_card(
                                    "Transforms",
                                    "save/restore isolates rotated and translated drawing passes.",
                                    transform_scene(),
                                ),
                                self.example_card(
                                    "Clip",
                                    "Rect and path clips gate later drawing commands without exposing retained nodes.",
                                    clip_scene(),
                                ),
                                self.example_card(
                                    "Blend",
                                    "Global alpha and blend modes stay available through recorder state.",
                                    blend_scene(),
                                ),
                                self.example_card(
                                    "Recorder",
                                    "Shortcuts, stable IDs, clip scoping, and long-label overflow in one scene.",
                                    recorder_scene(),
                                ),
                                self.example_card(
                                    "Events",
                                    "Recorder-generated item IDs still participate in hover, click, wheel, and drag dispatch.",
                                    interaction_scene(),
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
        .title("tgui Canvas Recorder Gallery")
        .window_size(dp(1480.0), dp(1080.0))
        .with_view_model(CanvasVm::new)
        .root_view(CanvasVm::view)
        .run()
}
