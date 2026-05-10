use super::{centered_text_frame, resolved_text_metrics, ResolvedWidgetKind, SELECT_ARROW_ICON};
use std::collections::HashMap;

use crate::animation::{AnimationCoordinator, AnimationEngine};
use crate::foundation::binding::{
    DependencyOwner, DependencyPhase, InvalidationSignal, ViewModelContext,
};
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, CommandContext, ValueCommand};
use crate::media::{MediaManager, MediaSource};
use crate::text::font::{FontCatalog, FontManager, TextFontRequest};
use crate::ui::layout::{Axis, Insets, Overflow};
use crate::ui::theme::{Stateful, Theme};
use crate::ui::unit::{dp, sp, Dp, UnitContext};
use crate::ui::widget::common::{ContainerKind, Rect, WidgetKind};
use crate::ui::widget::style::infer_theme_mode;
use crate::ui::widget::{
    BackgroundGradientStop, BackgroundImage, BackgroundLinearGradient, BackgroundRadialGradient,
};
use crate::ui::widget::{
    ButtonStyle, Canvas, CanvasItem, CanvasPath, CanvasStroke, CanvasStyle, Checkbox, ClipMask,
    ContainerStyle, Element, Image, Input, InputStyle, PathBuilder, Point, Radio, RadioGroup,
    RadioOption, ScrollbarAxis, ScrollbarHandle, Select, SelectOption, Stack, Switch, SwitchStyle,
    Text, TextEditState, TextWidgetStyle, Textarea, TextareaStyle, WidgetStateMap, WidgetTree,
};
#[cfg(feature = "video")]
use crate::video::backend::{
    BackendSharedState, VideoBackend, DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
};
#[cfg(feature = "video")]
use crate::video::{PlaybackState, VideoController, VideoMetrics, VideoSize, VideoSurface};

const ONE_BY_ONE_GIF: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0xFF, 0xFF, 0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x01, 0x4C,
    0x00, 0x3B,
];

fn stateful<T: Clone>(value: T) -> Stateful<T> {
    Stateful {
        normal: value.clone(),
        hovered: value.clone(),
        pressed: value.clone(),
        disabled: value,
    }
}

fn text_style(
    mode: crate::theme::ResolvedThemeMode,
    size: Option<crate::ui::unit::Sp>,
) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    if let Some(size) = size {
        style.typography.size = size;
    }
    style
}

fn container_style(
    mode: crate::theme::ResolvedThemeMode,
    background: Option<Color>,
    brush: Option<crate::ui::widget::BackgroundBrush>,
    image: Option<BackgroundImage>,
    blur: Option<Dp>,
    border: Option<(Dp, Color)>,
    radius: Option<Dp>,
    offset: Option<Point>,
) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.background = background.map(Into::into);
    style.surface.background_brush = brush.map(Into::into);
    style.surface.background_image = image.map(Into::into);
    if let Some(blur) = blur {
        style.surface.background_blur = blur.into();
    }
    if let Some((width, color)) = border {
        style.surface.border_width = Some(width.into());
        style.surface.border_color = Some(color.into());
    }
    if let Some(radius) = radius {
        style.surface.border_radius = Some(radius.into());
    }
    if let Some(offset) = offset {
        style.surface.offset = offset.into();
    }
    style
}

fn canvas_style(mode: crate::theme::ResolvedThemeMode, radius: Dp) -> CanvasStyle {
    let mut style = CanvasStyle::default_for(mode);
    style.surface.border_radius = Some(radius.into());
    style
}

fn button_style(
    mode: crate::theme::ResolvedThemeMode,
    radius: Option<Dp>,
    border_width: Option<Dp>,
    border_color: Option<Color>,
) -> ButtonStyle {
    let mut style =
        ButtonStyle::default_for(mode, crate::ui::widget::common::ButtonVariantKind::Primary);
    if let Some(radius) = radius {
        style.radius = radius.into();
    }
    if let Some(border_width) = border_width {
        style.border_width = border_width.into();
    }
    if let Some(border_color) = border_color {
        style.border = stateful(border_color.into());
    }
    style
}

fn switch_style(
    mode: crate::theme::ResolvedThemeMode,
    active_background: Color,
    inactive_background: Color,
    active_thumb: Option<Color>,
    inactive_thumb: Option<Color>,
) -> SwitchStyle {
    let mut style = SwitchStyle::default_for(mode);
    style.track_checked = stateful(active_background.into());
    style.track = stateful(inactive_background.into());
    if let Some(active_thumb) = active_thumb {
        style.thumb_checked = stateful(active_thumb.into());
    }
    if let Some(inactive_thumb) = inactive_thumb {
        style.thumb = stateful(inactive_thumb.into());
    }
    style
}

fn resolved_theme_mode(theme: &Theme) -> crate::theme::ResolvedThemeMode {
    infer_theme_mode(theme)
}

fn default_checkbox_style(
    theme: &Theme,
    state: crate::ui::theme::WidgetState,
    checked: bool,
) -> super::ResolvedCheckboxStyle {
    super::resolve_checkbox_style(
        &super::WidgetCheckboxStyle::default_for(resolved_theme_mode(theme)),
        state,
        checked,
        theme,
    )
}

fn default_radio_style(
    theme: &Theme,
    state: crate::ui::theme::WidgetState,
    checked: bool,
) -> super::ResolvedRadioStyle {
    super::resolve_radio_style(
        &super::WidgetRadioStyle::default_for(resolved_theme_mode(theme)),
        state,
        checked,
        theme,
    )
}

fn default_button_style(
    theme: &Theme,
    state: crate::ui::theme::WidgetState,
    variant: crate::ui::widget::common::ButtonVariantKind,
) -> super::ResolvedButtonStyle {
    super::resolve_button_style(
        &ButtonStyle::default_for(resolved_theme_mode(theme), variant),
        state,
        theme,
    )
}

fn default_switch_style(theme: &Theme) -> super::WidgetSwitchStyle {
    super::WidgetSwitchStyle::default_for(resolved_theme_mode(theme))
}

fn default_select_style(
    theme: &Theme,
    state: crate::ui::theme::WidgetState,
) -> super::ResolvedSelectStyle {
    super::resolve_select_style(
        &super::WidgetSelectStyle::default_for(resolved_theme_mode(theme)),
        state,
        theme,
    )
}

#[test]
fn centers_text_using_actual_render_height() {
    let inner = Rect::new(12.0, 8.0, 180.0, 24.0);
    let frame = centered_text_frame(inner, 56.0, 18.0, 18.0, false);

    assert_eq!(frame.x, 12.0);
    assert_eq!(frame.y, 11.0);
    assert_eq!(frame.width, 56.0);
    assert_eq!(frame.height, 18.0);
}

#[test]
fn centers_text_horizontally_when_requested() {
    let inner = Rect::new(12.0, 8.0, 180.0, 24.0);
    let frame = centered_text_frame(inner, 56.0, 18.0, 18.0, true);

    assert_eq!(frame.x, 74.0);
    assert_eq!(frame.y, 11.0);
    assert_eq!(frame.width, 56.0);
    assert_eq!(frame.height, 18.0);
}

#[test]
fn text_background_matches_measured_text_width() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let background = crate::foundation::color::Color::RED;
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(52.0), dp(52.0)).center().child(
            Text::new("A").style(move |mode| {
                let mut style = text_style(mode, None);
                style.surface.background = Some(background.into());
                style
            }),
        ));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    let text = rendered
        .primitives
        .texts
        .first()
        .expect("text primitive should exist");
    let background_shape = rendered
        .primitives
        .shapes
        .iter()
        .find(|primitive| primitive.color == background && primitive.rect.width.get() < 52.0)
        .expect("text background should exist");

    assert!((background_shape.rect.width.get() - text.frame.width.get()).abs() <= 1.0);
    assert!((background_shape.rect.height.get() - text.frame.height.get()).abs() <= 1.0);
}

#[test]
fn larger_font_sizes_scale_default_line_height() {
    let theme = Theme::default();
    let mut text = Text::new("Background Effects Gallery");
    let style = text_style(resolved_theme_mode(&theme), Some(sp(30.0)));
    super::apply_text_widget_style(&mut text, &style);
    let (font_size, line_height, _) = resolved_text_metrics(&text, &theme, UnitContext::default());

    assert_eq!(font_size, 30.0);
    assert_eq!(line_height, 41.25);
}

#[test]
fn image_loading_placeholder_uses_image_background() {
    let background = Color::hexa(0x11223344);

    assert_eq!(
        super::media_loading_fill_color(true, None, background, true),
        background
    );
}

#[test]
fn image_loading_placeholder_defaults_to_transparent_white() {
    assert_eq!(
        super::media_loading_fill_color(true, None, Color::rgba(255, 255, 255, 0), true),
        Color::rgba(255, 255, 255, 0)
    );
}

#[test]
fn image_error_placeholder_keeps_error_color() {
    assert_eq!(
        super::media_loading_fill_color(false, Some("boom"), Color::WHITE, false),
        crate::media::media_placeholder_color(false, Some("boom"))
    );
}

#[test]
fn idle_media_placeholder_keeps_default_placeholder_color() {
    let background = Color::hexa(0xABCDEF12);

    assert_eq!(
        super::media_loading_fill_color(false, None, background, false),
        crate::media::media_placeholder_color(false, None)
    );
}

#[test]
fn canvas_without_explicit_size_uses_item_bounds_for_layout() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let canvas: Element<()> = Canvas::new(vec![CanvasItem::Path(
        CanvasPath::new(
            1_u64,
            PathBuilder::new()
                .move_to(0.0, 0.0)
                .line_to(80.0, 0.0)
                .line_to(80.0, 30.0)
                .line_to(0.0, 30.0)
                .close(),
        )
        .fill(Color::WHITE),
    )])
    .cursor(crate::ui::widget::CursorStyle::Pointer)
    .into();
    let canvas_id = canvas.id;
    let tree = WidgetTree::new(Stack::new().child(canvas));

    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    let widget_region = computed
        .hit_regions
        .iter()
        .find(|region| matches!(region.interaction, super::HitInteraction::Widget { id, .. } if id == canvas_id))
        .expect("canvas widget region should exist");
    assert_eq!(widget_region.rect.width, 80.0);
    assert_eq!(widget_region.rect.height, 30.0);
}

#[test]
fn background_brush_generates_brush_primitive() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(120.0), dp(80.0)).style(|mode| {
            container_style(
                mode,
                None,
                Some(
                    BackgroundLinearGradient::new(
                        Point::new(dp(0.0), dp(0.0)),
                        Point::new(dp(120.0), dp(80.0)),
                        vec![
                            BackgroundGradientStop::new(0.0, Color::hexa(0x38BDF8FF)),
                            BackgroundGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                        ],
                    )
                    .into(),
                ),
                None,
                None,
                None,
                Some(dp(12.0)),
                None,
            )
        }));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.brushes.len(), 1);
    assert!(matches!(
        rendered.primitives.brushes[0].brush,
        crate::ui::widget::BackgroundBrush::LinearGradient(_)
    ));
}

#[test]
fn background_brush_takes_priority_over_background_color() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(120.0), dp(80.0)).style(|mode| {
            container_style(
                mode,
                Some(Color::hexa(0xEF4444FF)),
                Some(
                    BackgroundRadialGradient::new(
                        Point::new(dp(60.0), dp(40.0)),
                        dp(72.0),
                        vec![
                            BackgroundGradientStop::new(0.0, Color::hexa(0xFFFFFFAA)),
                            BackgroundGradientStop::new(1.0, Color::hexa(0x2563EB00)),
                        ],
                    )
                    .into(),
                ),
                None,
                None,
                None,
                None,
                None,
            )
        }));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.brushes.len(), 1);
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .all(|shape| shape.color != Color::hexa(0xEF4444FF)));
}

#[test]
fn background_blur_is_emitted_before_background_fill() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(120.0), dp(80.0)).style(|mode| {
            container_style(
                mode,
                Some(Color::hexa(0x112233AA)),
                None,
                None,
                Some(dp(18.0)),
                None,
                None,
                None,
            )
        }));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.backdrop_blurs.len(), 1);
    assert!(matches!(
        rendered.primitives.commands.first(),
        Some(crate::ui::widget::RenderCommand::BackdropBlur(_))
    ));
}

#[test]
fn background_image_produces_texture_primitive() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(64.0), dp(64.0)).style(|mode| {
            container_style(
                mode,
                None,
                None,
                Some(BackgroundImage::from_bytes(ONE_BY_ONE_GIF)),
                None,
                None,
                None,
                None,
            )
        }));

    let rendered = wait_for_rendered_output(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        Rect::new(0.0, 0.0, 64.0, 64.0),
    );

    assert_eq!(rendered.primitives.textures.len(), 1);
    assert_eq!(rendered.primitives.textures[0].frame.width, 64.0);
    assert_eq!(rendered.primitives.textures[0].frame.height, 64.0);
}

#[test]
fn background_image_loading_failure_keeps_base_background_without_placeholder_text() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let fallback = Color::hexa(0x112233FF);
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(80.0), dp(50.0)).style(move |mode| {
            container_style(
                mode,
                Some(fallback),
                None,
                Some(BackgroundImage::new(MediaSource::bytes(
                    b"not-an-image".as_slice(),
                ))),
                None,
                None,
                None,
                None,
            )
        }));

    let rendered = wait_for_rendered_output(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        Rect::new(0.0, 0.0, 80.0, 50.0),
    );

    assert!(rendered.primitives.textures.is_empty());
    assert!(rendered.primitives.texts.is_empty());
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == fallback));
}

#[test]
fn background_image_renders_between_blur_and_brush_overlay() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(96.0), dp(72.0)).style(|mode| {
            container_style(
                mode,
                Some(Color::hexa(0x0F172AFF)),
                Some(
                    BackgroundLinearGradient::new(
                        Point::new(dp(0.0), dp(0.0)),
                        Point::new(dp(96.0), dp(72.0)),
                        vec![
                            BackgroundGradientStop::new(0.0, Color::hexa(0xFFFFFF33)),
                            BackgroundGradientStop::new(1.0, Color::hexa(0x00000033)),
                        ],
                    )
                    .into(),
                ),
                Some(BackgroundImage::from_bytes(ONE_BY_ONE_GIF)),
                Some(dp(10.0)),
                Some((dp(1.0), Color::WHITE)),
                None,
                None,
            )
        }));

    let rendered = wait_for_rendered_output(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        Rect::new(0.0, 0.0, 96.0, 72.0),
    );

    let commands = &rendered.primitives.commands;
    assert!(matches!(
        commands.get(0),
        Some(crate::ui::widget::RenderCommand::BackdropBlur(_))
    ));
    assert!(matches!(
        commands.get(1),
        Some(crate::ui::widget::RenderCommand::Shape(_))
    ));
    assert!(matches!(
        commands.get(2),
        Some(crate::ui::widget::RenderCommand::Texture(_))
    ));
    assert!(matches!(
        commands.get(3),
        Some(crate::ui::widget::RenderCommand::Brush(_))
    ));
}

#[test]
fn background_image_texture_uses_corner_radius() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(64.0), dp(64.0)).style(|mode| {
            container_style(
                mode,
                None,
                None,
                Some(BackgroundImage::from_bytes(ONE_BY_ONE_GIF)),
                None,
                None,
                Some(dp(18.0)),
                None,
            )
        }));

    let rendered = wait_for_rendered_output(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        Rect::new(0.0, 0.0, 64.0, 64.0),
    );

    assert_eq!(rendered.primitives.textures.len(), 1);
    assert_eq!(rendered.primitives.textures[0].corner_radius, 18.0);
}

#[test]
fn background_brush_keeps_clip_rect() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Stack::new()
            .size(dp(100.0), dp(100.0))
            .overflow(Overflow::Hidden)
            .child(Stack::new().size(dp(120.0), dp(80.0)).style(|mode| {
                container_style(
                    mode,
                    None,
                    Some(
                        BackgroundLinearGradient::new(
                            Point::new(dp(0.0), dp(0.0)),
                            Point::new(dp(120.0), dp(80.0)),
                            vec![
                                BackgroundGradientStop::new(0.0, Color::hexa(0x14B8A6FF)),
                                BackgroundGradientStop::new(1.0, Color::hexa(0x0F766EFF)),
                            ],
                        )
                        .into(),
                    ),
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            })),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.brushes.len(), 1);
    assert_eq!(
        rendered.primitives.brushes[0].clip_rect,
        Some(Rect::new(0.0, 0.0, 100.0, 100.0))
    );
}

#[test]
fn canvas_renders_fill_and_stroke_meshes() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Canvas::new(vec![CanvasItem::Path(
            CanvasPath::new(
                1_u64,
                PathBuilder::new()
                    .move_to(10.0, 10.0)
                    .line_to(100.0, 10.0)
                    .line_to(100.0, 60.0)
                    .line_to(10.0, 60.0)
                    .close(),
            )
            .fill(Color::hexa(0x22C55EFF))
            .stroke(CanvasStroke::new(dp(4.0), Color::WHITE)),
        )])
        .size(dp(120.0), dp(80.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.meshes.len(), 2);
    assert!(!rendered.primitives.commands.is_empty());
}

#[test]
fn canvas_border_radius_clips_item_meshes() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Canvas::new(vec![CanvasItem::Path(
            CanvasPath::new(1_u64, PathBuilder::new().rect(0.0, 0.0, 120.0, 80.0))
                .fill(Color::hexa(0x22C55EFF)),
        )])
        .size(dp(120.0), dp(80.0))
        .style(|mode| canvas_style(mode, dp(18.0))),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(!rendered.primitives.meshes.is_empty());
    assert!(rendered.primitives.meshes.iter().all(|mesh| {
        mesh.clip_mask
            == Some(ClipMask {
                rect: Rect::new(0.0, 0.0, 120.0, 80.0),
                corner_radius: 18.0,
            })
    }));
}

#[test]
fn canvas_hit_testing_prefers_topmost_item() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Canvas::new(vec![
            CanvasItem::Path(
                CanvasPath::new(
                    1_u64,
                    PathBuilder::new()
                        .move_to(0.0, 0.0)
                        .line_to(80.0, 0.0)
                        .line_to(80.0, 80.0)
                        .line_to(0.0, 80.0)
                        .close(),
                )
                .fill(Color::hexa(0x1D4ED8FF)),
            ),
            CanvasItem::Path(
                CanvasPath::new(
                    2_u64,
                    PathBuilder::new()
                        .move_to(20.0, 20.0)
                        .line_to(90.0, 20.0)
                        .line_to(90.0, 90.0)
                        .line_to(20.0, 90.0)
                        .close(),
                )
                .fill(Color::hexa(0xF97316FF)),
            ),
        ])
        .size(dp(120.0), dp(120.0))
        .on_item_click(ValueCommand::new(|_: &mut (), _| {})),
    );

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 120.0),
        Some(Point::new(dp(30.0), dp(30.0))),
        None,
    );

    assert!(matches!(
        hit,
        Some(super::HitInteraction::CanvasItem { item_id, .. }) if item_id == 2_u64.into()
    ));
}

fn test_media() -> MediaManager {
    MediaManager::new(InvalidationSignal::new())
}

fn wait_for_rendered_output(
    tree: &WidgetTree<()>,
    font_manager: &FontManager,
    theme: &Theme,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    viewport: Rect,
) -> super::RenderedWidgetScene {
    for _ in 0..150 {
        let rendered = tree.render_output(
            font_manager,
            theme,
            media,
            animations,
            None,
            None,
            &HashMap::new(),
            viewport,
            None,
            None,
            None,
            None,
            false,
        );
        if !rendered.primitives.textures.is_empty() {
            return rendered;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    tree.render_output(
        font_manager,
        theme,
        media,
        animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    )
}

fn test_context() -> ViewModelContext {
    ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default())
}

#[test]
fn text_signal_records_layout_and_scene_dependencies() {
    let ctx = test_context();
    let content = ctx.state(String::from("tracked"));
    let text: Element<()> = Text::new(content.signal()).into();
    let widget_id = text.id;
    let tree = WidgetTree::new(text);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(layout.dependencies().contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Layout,
    }));

    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &Theme::default(),
        &media,
        &mut animations,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(computed.dependencies.contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Scene,
    }));
}

#[test]
fn dynamic_children_signal_records_structure_dependency() {
    let ctx = test_context();
    let show = ctx.state(true);
    let container: Element<()> = Stack::new()
        .child(show.signal().map(|show| {
            if show {
                Text::new("shown")
            } else {
                Text::new("hidden")
            }
        }))
        .into();
    let widget_id = container.id;
    let tree = WidgetTree::new(container);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(layout.dependencies().contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Structure,
    }));
}

#[test]
fn keyed_dynamic_children_reuse_widget_ids_across_reorder_patch() {
    let ctx = test_context();
    let reversed = ctx.state(false);
    let container: Element<()> = Stack::<()>::new()
        .child(reversed.signal().map(|reversed| {
            if reversed {
                vec![
                    Element::from(Text::new("second").key("second")),
                    Element::from(Text::new("first").key("first")),
                ]
            } else {
                vec![
                    Element::from(Text::new("first").key("first")),
                    Element::from(Text::new("second").key("second")),
                ]
            }
        }))
        .into();
    let container_id = container.id;
    let tree = WidgetTree::new(container);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let viewport = Rect::new(0.0, 0.0, 200.0, 120.0);
    let mut animations = AnimationEngine::default();

    let mut layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        viewport,
    );

    let initial_ids = match &layout.resolved_root.kind {
        ResolvedWidgetKind::Container { children, .. } => {
            children.iter().map(|child| child.id).collect::<Vec<_>>()
        }
        _ => panic!("stack root should resolve to a container"),
    };

    reversed.set(true);
    let removed = layout
        .patch_layout_roots(
            &[container_id],
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
        )
        .expect("keyed reorder should patch successfully");

    assert!(removed.is_empty());
    let reordered_ids = match &layout.resolved_root.kind {
        ResolvedWidgetKind::Container { children, .. } => {
            children.iter().map(|child| child.id).collect::<Vec<_>>()
        }
        _ => panic!("stack root should remain a container"),
    };
    assert_eq!(reordered_ids, vec![initial_ids[1], initial_ids[0]]);
}

#[test]
fn canvas_items_signal_records_layout_and_scene_dependencies() {
    let ctx = test_context();
    let expanded = ctx.state(false);
    let canvas: Element<()> = Canvas::new(expanded.signal().map(|expanded| {
        let width = if expanded { 96.0 } else { 48.0 };
        vec![CanvasItem::Path(
            CanvasPath::new(
                1_u64,
                PathBuilder::new()
                    .move_to(0.0, 0.0)
                    .line_to(width, 0.0)
                    .line_to(width, 24.0)
                    .line_to(0.0, 24.0)
                    .close(),
            )
            .fill(Color::WHITE),
        )]
    }))
    .into();
    let widget_id = canvas.id;
    let tree = WidgetTree::new(canvas);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(!layout.dependencies().has_global_dependency());
    assert!(layout.dependencies().contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Layout,
    }));

    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &Theme::default(),
        &media,
        &mut animations,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(!computed.dependencies.has_global_dependency());
    assert!(computed.dependencies.contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Scene,
    }));
}

#[test]
fn multiline_textarea_layout_is_content_independent() {
    let ctx = test_context();
    let auto_wrap = ctx.state(true);
    let textarea: Element<()> = Textarea::new("tracked text")
        .auto_wrap(auto_wrap.signal())
        .into();
    let tree = WidgetTree::new(textarea);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(!layout.dependencies().has_global_dependency());
    assert_eq!(layout.dependencies().dependency_count(), 0);
}

#[test]
fn textarea_non_focused_render_reuses_stable_layout_snapshot() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let content = "line 0\nline 1\nline 2";
    let textarea: Element<()> = Textarea::new(content).height(dp(52.0)).into();
    let widget_id = textarea.id;
    let tree = WidgetTree::new(textarea);
    let viewport = Rect::new(0.0, 0.0, 220.0, 52.0);
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        viewport,
    );

    let baseline = tree.collect_scene_from_layout_with_focus_value(
        &font_manager,
        &layout,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
    );
    let baseline_region = baseline
        .scroll_regions
        .iter()
        .find(|region| region.id == widget_id)
        .expect("textarea scroll region should exist");

    let style = TextareaStyle::default_for(infer_theme_mode(&theme));
    let text = super::text_with_typography(content, &style.text_style);
    let (font_size, line_height, letter_spacing) =
        resolved_text_metrics(&text, &theme, UnitContext::default());
    let request = TextFontRequest {
        preferred_font: text.font_family.as_deref().or(theme
            .typography
            .body
            .font_family
            .as_deref()),
        weight: text.font_weight.unwrap_or(theme.typography.body.weight),
    };
    let alternate_layout = font_manager.measure_text_layout(
        content,
        request,
        font_size,
        line_height * 2.0,
        letter_spacing,
    );
    let overrides = HashMap::from([(
        widget_id,
        super::TextInputLayoutOverride {
            revision: 1,
            text: content,
            layout: &alternate_layout,
        },
    )]);

    let overridden = tree.collect_scene_from_layout_with_focus_value(
        &font_manager,
        &layout,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        Some(&overrides),
        None,
        None,
        false,
    );
    let overridden_region = overridden
        .scroll_regions
        .iter()
        .find(|region| region.id == widget_id)
        .expect("textarea scroll region should exist");

    assert!(overridden_region.content_bounds.height > baseline_region.content_bounds.height);
    assert_eq!(
        overridden_region.content_bounds.height.get(),
        alternate_layout
            .height
            .max(overridden_region.content_viewport.height.get())
    );
}

#[test]
fn textarea_show_scrollbar_signal_only_records_scene_dependency() {
    let ctx = test_context();
    let show_scrollbar = ctx.state(false);
    let textarea: Element<()> = Textarea::new("line 0\nline 1\nline 2\nline 3")
        .height(dp(52.0))
        .show_scrollbar(show_scrollbar.signal())
        .into();
    let widget_id = textarea.id;
    let tree = WidgetTree::new(textarea);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(!layout.dependencies().has_global_dependency());
    assert_eq!(layout.dependencies().dependency_count(), 0);

    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &Theme::default(),
        &media,
        &mut animations,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(!computed.dependencies.has_global_dependency());
    assert_eq!(computed.dependencies.dependency_count(), 2);
    assert!(computed.dependencies.contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Scene,
    }));
}

#[cfg(feature = "video")]
fn test_video_controller(snapshot: crate::video::VideoSurfaceSnapshot) -> VideoController {
    struct StaticVideoBackend;

    impl VideoBackend for StaticVideoBackend {
        fn load(&self, _source: crate::video::VideoSource) -> Result<(), crate::TguiError> {
            Ok(())
        }

        fn play(&self) {}
        fn pause(&self) {}
        fn seek(&self, _position: std::time::Duration) {}
        fn set_volume(&self, _volume: f32) {}
        fn set_muted(&self, _muted: bool) {}
        fn set_buffer_memory_limit_bytes(&self, _bytes: u64) {}
        fn current_frame(&self) -> Option<std::sync::Arc<crate::media::TextureFrame>> {
            None
        }
        fn shutdown(&self) {}
    }

    let ctx = test_context();
    let shared = BackendSharedState {
        playback_state: ctx.state(PlaybackState::Ready),
        metrics: ctx.state(VideoMetrics {
            duration: Some(std::time::Duration::from_secs(30)),
            position: std::time::Duration::ZERO,
            buffered: Some(std::time::Duration::from_secs(30)),
            video_width: snapshot.intrinsic_size.width as u32,
            video_height: snapshot.intrinsic_size.height as u32,
        }),
        volume: ctx.state(1.0),
        muted: ctx.state(false),
        buffer_memory_limit_bytes: ctx.state(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
        video_size: ctx.state(VideoSize {
            width: snapshot.intrinsic_size.width as u32,
            height: snapshot.intrinsic_size.height as u32,
        }),
        error: ctx.state(snapshot.error.clone()),
        surface: ctx.state(snapshot),
    };
    VideoController::from_parts(shared, std::sync::Arc::new(StaticVideoBackend))
}

#[test]
fn clipped_children_keep_clip_rect_and_do_not_hit_outside_parent() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree = WidgetTree::new(
        Stack::new().child(
            Stack::new()
                .size(dp(100.0), dp(100.0))
                .style(|mode| {
                    container_style(
                        mode,
                        Some(Color::hexa(0x1E293BFF)),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                })
                .child(
                    Stack::new()
                        .size(dp(80.0), dp(80.0))
                        .style(|mode| {
                            container_style(
                                mode,
                                Some(Color::hexa(0x38BDF8FF)),
                                None,
                                None,
                                None,
                                None,
                                None,
                                Some(Point::new(dp(60.0), dp(0.0))),
                            )
                        })
                        .on_click(Command::new(|_: &mut ()| {})),
                ),
        ),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(
        rendered
            .primitives
            .shapes
            .last()
            .and_then(|primitive| primitive.clip_rect),
        Some(Rect::new(0.0, 0.0, 100.0, 100.0))
    );

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Some(Point::new(dp(120.0), dp(20.0))),
        None,
    );
    assert!(hit.is_none());
}

#[test]
fn wrapped_flex_align_start_packs_lines_from_cross_axis_start() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let child_color = crate::foundation::color::Color::hexa(0x22C55EFF);
    let tree: WidgetTree<()> = WidgetTree::new(
        crate::ui::widget::Flex::horizontal()
            .wrap(crate::ui::layout::Wrap::Wrap)
            .align(crate::ui::layout::Align::Start)
            .justify(crate::ui::layout::Justify::Start)
            .gap(dp(10.0))
            .child([
                Stack::new().size(dp(60.0), dp(40.0)).style(move |mode| {
                    container_style(mode, Some(child_color), None, None, None, None, None, None)
                }),
                Stack::new().size(dp(60.0), dp(40.0)).style(move |mode| {
                    container_style(mode, Some(child_color), None, None, None, None, None, None)
                }),
                Stack::new().size(dp(60.0), dp(40.0)).style(move |mode| {
                    container_style(mode, Some(child_color), None, None, None, None, None, None)
                }),
            ]),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 140.0, 240.0),
        None,
        None,
        None,
        None,
        false,
    );
    let child_rects: Vec<_> = rendered
        .primitives
        .shapes
        .iter()
        .filter(|shape| shape.color == child_color)
        .map(|shape| shape.rect)
        .collect();

    assert_eq!(child_rects.len(), 3);
    assert_eq!(child_rects[0], Rect::new(0.0, 0.0, 60.0, 40.0));
    assert_eq!(child_rects[1], Rect::new(70.0, 0.0, 60.0, 40.0));
    assert_eq!(child_rects[2], Rect::new(0.0, 50.0, 60.0, 40.0));
}

#[test]
fn scroll_offsets_are_clamped_to_content_bounds() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let scroller: super::Element<()> = Stack::new()
        .size(dp(100.0), dp(100.0))
        .overflow_y(Overflow::Scroll)
        .style(|mode| {
            container_style(
                mode,
                Some(crate::foundation::color::Color::hexa(0x111827FF)),
                None,
                None,
                None,
                Some((dp(4.0), crate::foundation::color::Color::WHITE)),
                None,
                None,
            )
        })
        .child(Stack::new().size(dp(100.0), dp(300.0)).style(|mode| {
            container_style(
                mode,
                Some(crate::foundation::color::Color::hexa(0x22C55EFF)),
                None,
                None,
                None,
                None,
                None,
                None,
            )
        }))
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(Stack::new().child(scroller));

    let mut scroll_offsets = HashMap::new();
    scroll_offsets.insert(scroller_id, Point::new(dp(0.0), dp(500.0)));
    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &scroll_offsets,
        Rect::new(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    let region = rendered
        .scroll_regions
        .into_iter()
        .find(|region| region.id == scroller_id)
        .expect("scroll region should exist");
    assert_eq!(region.content_viewport, Rect::new(4.0, 4.0, 92.0, 92.0));
    assert_eq!(region.scroll_offset.y, 204.0);
    assert_eq!(region.max_offset().y, 204.0);
}

#[test]
fn scroll_content_bounds_include_container_bottom_padding() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let scroller: super::Element<()> = Stack::new()
        .size(dp(100.0), dp(100.0))
        .padding(Insets::all(dp(20.0)))
        .overflow_y(Overflow::Scroll)
        .child(Stack::new().size(dp(60.0), dp(120.0)).style(|mode| {
            container_style(
                mode,
                Some(crate::foundation::color::Color::hexa(0x22C55EFF)),
                None,
                None,
                None,
                None,
                None,
                None,
            )
        }))
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    let region = rendered
        .scroll_regions
        .into_iter()
        .find(|region| region.id == scroller_id)
        .expect("scroll region should exist");
    assert_eq!(region.content_viewport, Rect::new(0.0, 0.0, 100.0, 100.0));
    assert_eq!(region.content_bounds.bottom(), dp(160.0));
    assert_eq!(region.max_offset().y, 60.0);
}

#[test]
fn overflow_clips_children_to_inside_of_border() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree = WidgetTree::new(
        Stack::<()>::new()
            .size(dp(100.0), dp(100.0))
            .overflow(Overflow::Hidden)
            .style(|mode| {
                container_style(
                    mode,
                    None,
                    None,
                    None,
                    None,
                    Some((dp(4.0), crate::foundation::color::Color::WHITE)),
                    None,
                    None,
                )
            })
            .child(Stack::new().size(dp(100.0), dp(100.0)).style(|mode| {
                container_style(
                    mode,
                    Some(crate::foundation::color::Color::BLACK),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            })),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    let child_shape = rendered
        .primitives
        .shapes
        .iter()
        .find(|primitive| primitive.color == crate::foundation::color::Color::BLACK)
        .expect("child shape should exist");
    assert_eq!(child_shape.clip_rect, Some(Rect::new(4.0, 4.0, 92.0, 92.0)));
}

#[test]
fn rounded_overflow_clips_children_with_parent_corner_mask() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree = WidgetTree::new(
        Stack::<()>::new()
            .size(dp(100.0), dp(100.0))
            .style(|mode| {
                container_style(
                    mode,
                    Some(crate::foundation::color::Color::WHITE),
                    None,
                    None,
                    None,
                    None,
                    Some(dp(18.0)),
                    None,
                )
            })
            .overflow(Overflow::Hidden)
            .child(Stack::new().size(dp(100.0), dp(40.0)).style(|mode| {
                container_style(
                    mode,
                    Some(crate::foundation::color::Color::BLACK),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            })),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    let child_shape = rendered
        .primitives
        .shapes
        .iter()
        .find(|primitive| primitive.color == crate::foundation::color::Color::BLACK)
        .expect("child shape should exist");
    assert_eq!(
        child_shape.clip_mask,
        Some(ClipMask {
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
            corner_radius: 18.0,
        })
    );
}

#[test]
fn scroll_containers_render_scrollbar_track_and_thumb() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let scroller: super::Element<()> = Stack::new()
        .size(dp(120.0), dp(120.0))
        .overflow_y(Overflow::Scroll)
        .style(|mode| {
            let mut style = ContainerStyle::default_for(mode);
            style.scrollbar.thumb_color = Some(crate::foundation::color::Color::BLACK);
            style.scrollbar.track_color = Some(crate::foundation::color::Color::WHITE);
            style.scrollbar.hover_thumb_color =
                Some(crate::foundation::color::Color::hexa(0x112233FF));
            style.scrollbar.active_thumb_color =
                Some(crate::foundation::color::Color::hexa(0x445566FF));
            style
        })
        .child(Stack::new().size(dp(120.0), dp(260.0)).style(|mode| {
            container_style(
                mode,
                Some(crate::foundation::color::Color::hexa(0x1D4ED8FF)),
                None,
                None,
                None,
                None,
                None,
                None,
            )
        }))
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    let overlay_shapes = rendered.primitives.overlay_shapes;
    assert!(overlay_shapes
        .iter()
        .any(|primitive| primitive.color == crate::foundation::color::Color::WHITE));
    assert!(overlay_shapes
        .iter()
        .any(|primitive| primitive.color == crate::foundation::color::Color::BLACK));

    let hovered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        Some(ScrollbarHandle {
            id: scroller_id,
            axis: ScrollbarAxis::Vertical,
        }),
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(hovered
        .primitives
        .overlay_shapes
        .iter()
        .any(|primitive| primitive.color == crate::foundation::color::Color::hexa(0x112233FF)));

    let active = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        Some(ScrollbarHandle {
            id: scroller_id,
            axis: ScrollbarAxis::Vertical,
        }),
        Some(ScrollbarHandle {
            id: scroller_id,
            axis: ScrollbarAxis::Vertical,
        }),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(active
        .primitives
        .overlay_shapes
        .iter()
        .any(|primitive| primitive.color == crate::foundation::color::Color::hexa(0x445566FF)));
}

#[test]
fn binding_driven_children_relayout_when_child_count_changes() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let expanded = context.state(false);
    let tree = WidgetTree::new(Stack::<()>::new().child(expanded.signal().map(|value| {
        if value {
            vec![
                Element::from(Text::new("first")),
                Element::from(Text::new("second")),
            ]
        } else {
            vec![Element::from(Text::new("first"))]
        }
    })));

    let mut animations = AnimationEngine::default();
    let compact = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert_eq!(compact.primitives.texts.len(), 1);

    expanded.set(true);
    let expanded_render = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert_eq!(expanded_render.primitives.texts.len(), 2);
}

#[test]
fn hit_testing_tracks_currently_resolved_children() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let visible = context.state(true);
    let clickable: Element<()> = Stack::new()
        .size(dp(40.0), dp(40.0))
        .style(|mode| {
            container_style(
                mode,
                Some(crate::foundation::color::Color::WHITE),
                None,
                None,
                None,
                None,
                None,
                None,
            )
        })
        .on_click(Command::new(|_: &mut ()| {}))
        .into();
    let tree = WidgetTree::new(Stack::<()>::new().size(dp(100.0), dp(100.0)).child(
        visible.signal().map(move |value| {
            if value {
                vec![clickable.clone()]
            } else {
                Vec::<Element<()>>::new()
            }
        }),
    ));

    let mut animations = AnimationEngine::default();
    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Some(Point::new(dp(10.0), dp(10.0))),
        None,
    );
    assert!(matches!(hit, Some(super::HitInteraction::Widget { .. })));

    visible.set(false);
    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Some(Point::new(dp(10.0), dp(10.0))),
        None,
    );
    assert!(hit.is_none());
}

#[derive(Default)]
struct ScopeChildVm {
    count: i32,
    checked: bool,
    selected_key: String,
    selected_value: String,
    canvas_hits: usize,
    context_hits: usize,
}

#[derive(Default)]
struct ScopeRootVm {
    child: ScopeChildVm,
    other: ScopeChildVm,
    root_count: i32,
}

fn scope_child(root: &mut ScopeRootVm) -> &mut ScopeChildVm {
    &mut root.child
}

fn scope_other(root: &mut ScopeRootVm) -> &mut ScopeChildVm {
    &mut root.other
}

#[test]
fn scoped_command_targets_child_view_model() {
    let child: Element<ScopeChildVm> = Stack::new()
        .on_click(Command::new(|vm: &mut ScopeChildVm| vm.count += 1))
        .into();
    let root = child.scope(scope_child);

    let command = root.interactions.on_click.expect("scoped command");
    let mut vm = ScopeRootVm::default();
    command.execute(&mut vm);

    assert_eq!(vm.child.count, 1);
    assert_eq!(vm.root_count, 0);
}

#[test]
fn scoped_context_command_receives_child_context() {
    let command = Command::new_with_context(
        |vm: &mut ScopeChildVm, _ctx: &CommandContext<ScopeChildVm>| {
            vm.context_hits += 1;
        },
    )
    .scope(std::sync::Arc::new(scope_child));

    let mut vm = ScopeRootVm::default();
    command.execute(&mut vm);

    assert_eq!(vm.child.context_hits, 1);
}

#[test]
fn scoped_lifecycle_command_targets_child_view_model() {
    let child: Element<ScopeChildVm> = Stack::new()
        .on_mount(Command::new(|vm: &mut ScopeChildVm| vm.count += 1))
        .into();
    let root = child.scope(scope_child);

    let command = root
        .lifecycle_events
        .on_mount
        .expect("scoped lifecycle command");
    let mut vm = ScopeRootVm::default();
    command.execute(&mut vm);

    assert_eq!(vm.child.count, 1);
    assert_eq!(vm.root_count, 0);
}

#[test]
fn checkbox_without_label_measures_to_theme_box_size() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(false));
    let expected = UnitContext::default().resolve_dp(
        default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), false).size,
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| { shape.rect.width == expected && shape.rect.height == expected }));
}

#[test]
fn checkbox_label_extends_measure_and_hit_region() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(false).label("Accept"));
    let checkbox_style =
        default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), false);
    let size = UnitContext::default().resolve_dp(checkbox_style.size);
    let gap = UnitContext::default().resolve_dp(checkbox_style.label_gap);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let label = rendered
        .primitives
        .texts
        .iter()
        .find(|text| text.content == "Accept")
        .expect("checkbox label should render");

    assert_eq!(label.frame.x, size + gap);
    assert!(label.frame.y >= Dp::ZERO);
    assert!(label.frame.y <= dp(12.0));
    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 40.0),
        Some(Point::new(label.frame.right() - 1.0, label.frame.y + 1.0)),
        None,
    );
    assert!(matches!(hit, Some(super::HitInteraction::Checkbox { .. })));
}

#[test]
fn checked_checkbox_renders_checked_background_and_checkmark() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(true));
    let checked_style =
        default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), true);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == checked_style.background));
    let checkmark = rendered
        .primitives
        .texts
        .iter()
        .find(|text| text.content == super::CHECKBOX_CHECKMARK_ICON)
        .expect("checked checkbox should render checkmark icon");
    assert_eq!(checkmark.color, Color::WHITE);
    assert!(checkmark.force_color);
    assert!(checkmark.font_family.is_some());
    let checkmark_center_x = checkmark.frame.x + checkmark.frame.width / 2.0;
    let checkmark_center_y = checkmark.frame.y + checkmark.frame.height / 2.0;
    assert!((checkmark_center_x - Dp::new(8.0)).abs().get() < 0.01);
    assert!((checkmark_center_y - Dp::new(21.0)).abs().get() < 0.01);
}

#[test]
fn hovered_checkbox_uses_primary_border_without_changing_background() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let checkbox: Element<()> = Checkbox::new(false).into();
    let checkbox_id = checkbox.id;
    let tree: WidgetTree<()> = WidgetTree::new(checkbox);
    let mut states = WidgetStateMap::default();
    states.set(
        checkbox_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        None,
        None,
        &states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let hovered_style = default_checkbox_style(
        &theme,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| { shape.stroke_width == 0.0 && shape.color == hovered_style.background }));
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| { shape.stroke_width > 0.0 && shape.color == hovered_style.border }));
}

#[test]
fn checkbox_checked_content_switches_without_animation() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let checkbox: Element<()> = Checkbox::new(false).into();
    let checkbox_id = checkbox.id;
    let unchecked_tree: WidgetTree<()> = WidgetTree::new(checkbox.clone());

    unchecked_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(!animations.has_active_animations());

    let mut checked_checkbox: Element<()> = Checkbox::new(true).into();
    checked_checkbox.id = checkbox_id;
    let checked_tree: WidgetTree<()> = WidgetTree::new(checked_checkbox);
    let checked = checked_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let checked_style =
        default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), true);
    let checked_fill = checked
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.stroke_width == 0.0 && shape.color == checked_style.background)
        .expect("checked fill should render immediately");
    let control_size = UnitContext::default().resolve_dp(checked_style.size);
    assert_eq!(checked_fill.rect.width, control_size);
    assert_eq!(checked_fill.rect.height, control_size);
    assert!(!animations.has_active_animations());

    let unchecked = unchecked_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(unchecked.primitives.shapes.iter().all(|shape| {
        shape.stroke_width == 0.0 && shape.color != checked_style.background
            || shape.stroke_width > 0.0
    }));
    assert!(!animations.has_active_animations());
}

#[test]
fn focused_unchecked_checkbox_keeps_default_box_colors() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let checkbox: Element<()> = Checkbox::new(false).into();
    let checkbox_id = checkbox.id;
    let tree: WidgetTree<()> = WidgetTree::new(checkbox);
    let mut states = WidgetStateMap::default();
    states.set(
        checkbox_id,
        crate::ui::theme::WidgetState {
            focused: true,
            ..Default::default()
        },
    );
    let default_style =
        default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), false);

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        None,
        None,
        &states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.stroke_width == 0.0 && shape.color == default_style.background));
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.stroke_width > 0.0 && shape.color == default_style.border));
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.stroke_width == theme.focus_ring.width.get()
            && shape.color == theme.focus_ring.color
            && shape.rect.width > dp(16.0)));
    assert!(rendered
        .primitives
        .texts
        .iter()
        .all(|text| text.content != super::CHECKBOX_CHECKMARK_ICON));
}

#[test]
fn disabled_checkbox_exposes_disabled_hit_for_cursor_only() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(false).disable(true));

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        Some(Point::new(4.0, 4.0)),
        None,
    );

    assert!(matches!(hit, Some(super::HitInteraction::Disabled { .. })));
}

#[test]
fn radio_without_label_measures_to_theme_control_size() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Radio::new(false));
    let expected = UnitContext::default().resolve_dp(
        default_radio_style(&theme, crate::ui::theme::WidgetState::default(), false).size,
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| { shape.rect.width == expected && shape.rect.height == expected }));
}

#[test]
fn radio_label_extends_measure_and_hit_region() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Radio::new(false).label("Email"));
    let radio_style = default_radio_style(&theme, crate::ui::theme::WidgetState::default(), false);
    let size = UnitContext::default().resolve_dp(radio_style.size);
    let gap = UnitContext::default().resolve_dp(radio_style.label_gap);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let label = rendered
        .primitives
        .texts
        .iter()
        .find(|text| text.content == "Email")
        .expect("radio label should render");

    assert_eq!(label.frame.x, size + gap);
    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 40.0),
        Some(Point::new(label.frame.right() - 1.0, label.frame.y + 1.0)),
        None,
    );
    assert!(matches!(hit, Some(super::HitInteraction::Radio { .. })));
}

#[test]
fn checked_radio_renders_indicator() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Radio::new(true));
    let checked_style = default_radio_style(&theme, crate::ui::theme::WidgetState::default(), true);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.color == checked_style.indicator));
}

#[test]
fn disabled_radio_exposes_disabled_hit_for_cursor_only() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Radio::new(false).disable(true));

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        Some(Point::new(4.0, 4.0)),
        None,
    );

    assert!(matches!(hit, Some(super::HitInteraction::Disabled { .. })));
}

#[test]
fn radio_group_renders_selected_option_and_dispatches_key_value() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<ScopeChildVm> = WidgetTree::new(
        RadioGroup::new(
            vec![
                ("email".to_string(), "Email".to_string()),
                ("sms".to_string(), "SMS".to_string()),
            ],
            "email".to_string(),
        )
        .on_change(ValueCommand::new(
            |vm: &mut ScopeChildVm, (key, value): (String, String)| {
                vm.selected_key = key;
                vm.selected_value = value;
            },
        )),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );
    let indicator =
        default_radio_style(&theme, crate::ui::theme::WidgetState::default(), true).indicator;
    assert_eq!(
        rendered
            .primitives
            .overlay_shapes
            .iter()
            .filter(|shape| shape.color == indicator)
            .count(),
        1
    );

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 80.0),
        Some(Point::new(4.0, 30.0)),
        None,
    );
    let mut vm = ScopeChildVm::default();
    match hit {
        Some(super::HitInteraction::Radio {
            on_change: Some(command),
            current,
            ..
        }) => {
            assert!(!current);
            command.execute(&mut vm, true);
        }
        _ => panic!("second radio should be hit"),
    }

    assert_eq!(vm.selected_key, "sms");
    assert_eq!(vm.selected_value, "SMS");
}

#[test]
fn radio_group_ignores_false_child_change_and_maps_direction() {
    let group: Element<ScopeChildVm> = RadioGroup::new(
        vec![
            ("email".to_string(), "Email".to_string()),
            ("sms".to_string(), "SMS".to_string()),
        ],
        "email".to_string(),
    )
    .horizontal()
    .on_change(ValueCommand::new(
        |vm: &mut ScopeChildVm, (key, value): (String, String)| {
            vm.selected_key = key;
            vm.selected_value = value;
        },
    ))
    .into();

    match &group.kind {
        WidgetKind::Container { layout, .. } => match &layout.kind {
            ContainerKind::Flex { direction, .. } => {
                assert_eq!(*direction, Axis::Horizontal);
            }
            _ => panic!("radio group should render as flex"),
        },
        _ => panic!("radio group should render as container"),
    }

    let tree = WidgetTree::new(group);
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 40.0),
        Some(Point::new(4.0, 4.0)),
        None,
    );
    let mut vm = ScopeChildVm::default();
    match hit {
        Some(super::HitInteraction::Radio {
            on_change: Some(command),
            current,
            ..
        }) => {
            assert!(current);
            command.execute(&mut vm, false);
        }
        _ => panic!("first radio should be hit"),
    }

    assert!(vm.selected_key.is_empty());
    assert!(vm.selected_value.is_empty());
}

#[test]
fn radio_group_disabled_option_exposes_disabled_hit_for_cursor_only() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<ScopeChildVm> = WidgetTree::new(
        RadioGroup::new(
            vec![
                RadioOption::new("email".to_string(), "Email".to_string()),
                RadioOption::new("sms".to_string(), "SMS".to_string()).disable(true),
            ],
            "email".to_string(),
        )
        .on_change(ValueCommand::new(
            |vm: &mut ScopeChildVm, (key, value): (String, String)| {
                vm.selected_key = key;
                vm.selected_value = value;
            },
        )),
    );

    let disabled_hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 80.0),
        Some(Point::new(4.0, 30.0)),
        None,
    );
    assert!(matches!(
        disabled_hit,
        Some(super::HitInteraction::Disabled { .. })
    ));

    let enabled_hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 80.0),
        Some(Point::new(4.0, 4.0)),
        None,
    );
    assert!(matches!(
        enabled_hit,
        Some(super::HitInteraction::Radio { .. })
    ));
}

#[test]
fn select_renders_placeholder_and_arrow_when_unselected() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Select::<(), String, String>::new(
            vec![SelectOption::new("email".to_string(), "Email".to_string())],
            None::<String>,
        )
        .placeholder("Choose one")
        .size(dp(180.0), dp(40.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .texts
        .iter()
        .any(|text| text.content == "Choose one"));
    assert!(rendered
        .primitives
        .texts
        .iter()
        .any(|text| text.content == SELECT_ARROW_ICON));
}

#[test]
fn disabled_select_exposes_disabled_hit_for_cursor_only() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Select::<(), String, String>::new(
            vec![SelectOption::new("email".to_string(), "Email".to_string())],
            None::<String>,
        )
        .disable(true)
        .size(dp(180.0), dp(40.0)),
    );

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 40.0),
        Some(Point::new(10.0, 10.0)),
        None,
    );
    assert!(matches!(hit, Some(super::HitInteraction::Disabled { .. })));
}

#[test]
fn focused_select_opens_upward_and_hits_enabled_and_disabled_options() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let select: Element<ScopeChildVm> = Select::new(
        vec![
            SelectOption::new("email".to_string(), "Email".to_string()),
            SelectOption::new("sms".to_string(), "SMS".to_string()).disable(true),
            SelectOption::new("phone".to_string(), "Phone".to_string()),
        ],
        Some("email".to_string()),
    )
    .on_change(ValueCommand::new(
        |vm: &mut ScopeChildVm, (key, value): (String, String)| {
            vm.selected_key = key;
            vm.selected_value = value;
        },
    ))
    .open(true)
    .size(dp(180.0), dp(40.0))
    .position_absolute()
    .top(dp(50.0))
    .into();
    let tree = WidgetTree::new(Stack::new().child(select));
    let widget_states = WidgetStateMap::default();

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 90.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.rect.y < dp(50.0) && shape.rect.height > dp(40.0)));

    let enabled_hit = tree.hit_test_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 90.0),
        Some(Point::new(8.0, 10.0)),
        None,
    );
    let mut vm = ScopeChildVm::default();
    match enabled_hit {
        Some(super::HitInteraction::SelectOption {
            on_select: Some(command),
            ..
        }) => command.execute(&mut vm),
        _ => panic!("enabled select option should be hit"),
    }
    assert_eq!(vm.selected_key, "email");
    assert_eq!(vm.selected_value, "Email");

    let disabled_hit = tree.hit_test_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 90.0),
        Some(Point::new(8.0, 45.0)),
        None,
    );
    assert!(matches!(
        disabled_hit,
        Some(super::HitInteraction::Disabled { .. })
    ));
}

#[test]
fn select_dropdown_escapes_parent_overflow_clip() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let select: Element<ScopeChildVm> = Select::new(
        vec![
            SelectOption::new("email".to_string(), "Email".to_string()),
            SelectOption::new("sms".to_string(), "SMS".to_string()),
        ],
        None::<String>,
    )
    .placeholder("Choose")
    .on_change(ValueCommand::new(
        |vm: &mut ScopeChildVm, (key, value): (String, String)| {
            vm.selected_key = key;
            vm.selected_value = value;
        },
    ))
    .open(true)
    .size(dp(180.0), dp(40.0))
    .into();
    let tree = WidgetTree::new(
        Stack::new()
            .size(dp(180.0), dp(45.0))
            .overflow(Overflow::Hidden)
            .child(select),
    );
    let widget_states = WidgetStateMap::default();

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 140.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.rect.y > dp(40.0) && shape.rect.bottom() > dp(45.0)));

    let hit = tree.hit_test_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 140.0),
        Some(Point::new(8.0, 58.0)),
        None,
    );
    let mut vm = ScopeChildVm::default();
    match hit {
        Some(super::HitInteraction::SelectOption {
            on_select: Some(command),
            ..
        }) => command.execute(&mut vm),
        _ => panic!("select option outside parent clip should be hit"),
    }
    assert_eq!(vm.selected_key, "email");
}

#[test]
fn select_dropdown_stays_above_later_media_placeholder() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let select: Element<ScopeChildVm> = Select::new(
        vec![
            SelectOption::new("email".to_string(), "Email".to_string()),
            SelectOption::new("sms".to_string(), "SMS".to_string()),
        ],
        None::<String>,
    )
    .open(true)
    .size(dp(180.0), dp(40.0))
    .into();
    let image_frame = Rect::new(0.0, 40.0, 180.0, 40.0);
    let tree = WidgetTree::new(
        crate::ui::widget::Flex::new(Axis::Vertical)
            .gap(dp(0.0))
            .child([
                select,
                Image::from_bytes(vec![0_u8; 4])
                    .size(dp(180.0), dp(40.0))
                    .into(),
            ]),
    );
    let widget_states = WidgetStateMap::default();

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 140.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(
        rendered
            .primitives
            .overlay_shapes
            .iter()
            .all(|shape| shape.rect != image_frame),
        "media placeholders should not render in the overlay layer"
    );
    assert!(
        rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.rect == image_frame),
        "media placeholder should still render in the normal scene"
    );
}

#[test]
fn select_dropdown_highlights_hovered_option() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let select: Element<ScopeChildVm> = Select::new(
        vec![
            SelectOption::new("email".to_string(), "Email".to_string()),
            SelectOption::new("sms".to_string(), "SMS".to_string()),
        ],
        None::<String>,
    )
    .open(true)
    .size(dp(180.0), dp(32.0))
    .into();
    let select_id = select.id;
    let tree = WidgetTree::new(Stack::new().child(select));
    let mut widget_states = WidgetStateMap::default();
    widget_states.set_select_option(
        select_id,
        1,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 140.0),
        None,
        None,
        None,
        None,
        false,
    );
    let hovered_options = rendered
        .primitives
        .overlay_shapes
        .iter()
        .filter(|shape| {
            shape.rect.y > dp(60.0)
                && shape.rect.height
                    == UnitContext::default().resolve_dp(
                        default_select_style(&theme, crate::ui::theme::WidgetState::default())
                            .option_height,
                    )
                && shape.color.a > 0
        })
        .collect::<Vec<_>>();

    assert_eq!(hovered_options.len(), 1);
}

#[test]
fn select_dropdown_hover_highlight_preserves_menu_corner_clip() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let select: Element<ScopeChildVm> = Select::new(
        vec![
            SelectOption::new("email".to_string(), "Email".to_string()),
            SelectOption::new("sms".to_string(), "SMS".to_string()),
        ],
        None::<String>,
    )
    .open(true)
    .size(dp(180.0), dp(32.0))
    .into();
    let select_id = select.id;
    let tree = WidgetTree::new(Stack::new().child(select));
    let mut widget_states = WidgetStateMap::default();
    widget_states.set_select_option(
        select_id,
        0,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 140.0),
        None,
        None,
        None,
        None,
        false,
    );
    let select_style = default_select_style(&theme, crate::ui::theme::WidgetState::default());
    let option_height = UnitContext::default().resolve_dp(select_style.option_height);
    let menu_radius = select_style.radius.get();
    let highlight = rendered
        .primitives
        .overlay_shapes
        .iter()
        .find(|shape| shape.rect.y > dp(20.0) && shape.rect.height == option_height)
        .expect("hovered option highlight should render");

    assert_eq!(
        highlight.clip_mask,
        Some(ClipMask {
            rect: Rect::new(
                highlight.rect.x,
                highlight.rect.y,
                highlight.rect.width,
                option_height * 2.0,
            ),
            corner_radius: menu_radius,
        })
    );
}

#[test]
fn scoped_value_commands_cover_switch_canvas_and_media() {
    let mut vm = ScopeRootVm::default();
    let switch: Element<ScopeChildVm> = Switch::new(false)
        .on_change(ValueCommand::new(|vm: &mut ScopeChildVm, value| {
            vm.checked = value;
        }))
        .into();
    let switch = switch.scope(scope_child);
    match switch.kind {
        WidgetKind::Switch {
            on_change: Some(command),
            ..
        } => command.execute(&mut vm, true),
        _ => panic!("switch command should be scoped"),
    }
    assert!(vm.child.checked);

    vm.child.checked = false;
    let checkbox: Element<ScopeChildVm> = Checkbox::new(false)
        .on_change(ValueCommand::new(|vm: &mut ScopeChildVm, value| {
            vm.checked = value;
        }))
        .into();
    let checkbox = checkbox.scope(scope_child);
    match checkbox.kind {
        WidgetKind::Checkbox {
            on_change: Some(command),
            ..
        } => command.execute(&mut vm, true),
        _ => panic!("checkbox command should be scoped"),
    }
    assert!(vm.child.checked);

    vm.child.checked = false;
    let radio: Element<ScopeChildVm> = Radio::new(false)
        .on_change(ValueCommand::new(|vm: &mut ScopeChildVm, value| {
            vm.checked = value;
        }))
        .into();
    let radio = radio.scope(scope_child);
    match radio.kind {
        WidgetKind::Radio {
            on_change: Some(command),
            ..
        } => command.execute(&mut vm, true),
        _ => panic!("radio command should be scoped"),
    }
    assert!(vm.child.checked);

    let canvas: Element<ScopeChildVm> = Canvas::new(Vec::<CanvasItem>::new())
        .on_item_click(ValueCommand::new(|vm: &mut ScopeChildVm, _event| {
            vm.canvas_hits += 1;
        }))
        .into();
    let canvas = canvas.scope(scope_child);
    match canvas.kind {
        WidgetKind::Canvas {
            item_interactions, ..
        } => item_interactions
            .on_click
            .expect("canvas item command")
            .execute(
                &mut vm,
                crate::ui::widget::CanvasPointerEvent {
                    item_id: 1_u64.into(),
                    button: None,
                    canvas_position: Point::ZERO,
                    scene_position: Point::ZERO,
                    local_position: Point::ZERO,
                },
            ),
        _ => panic!("canvas command should be scoped"),
    }
    assert_eq!(vm.child.canvas_hits, 1);

    let image = Image::from_path("missing-test-image.png")
        .on_loading(Command::new(|vm: &mut ScopeChildVm| vm.count += 10))
        .scope(scope_child);
    let media_command = image.media_events.on_loading.expect("media command");
    media_command.execute(&mut vm);
    assert_eq!(vm.child.count, 10);
}

#[test]
fn scoped_dynamic_children_resolve_to_root_commands() {
    let context = test_context();
    let show = context.state(true);
    let child_a: Element<ScopeChildVm> = Stack::new()
        .on_click(Command::new(|vm: &mut ScopeChildVm| vm.count += 1))
        .into();
    let child_b: Element<ScopeChildVm> = Stack::new()
        .on_click(Command::new(|vm: &mut ScopeChildVm| vm.count += 10))
        .into();

    let tree = WidgetTree::new(Stack::<ScopeRootVm>::new().child(show.signal().map(
        move |visible| {
            if visible {
                vec![child_a.clone().scope(scope_child)]
            } else {
                vec![child_b.clone().scope(scope_other)]
            }
        },
    )));

    let resolved = match &tree.root.kind {
        WidgetKind::Container { children, .. } => children[0].resolve(None),
        _ => panic!("root should be a container"),
    };

    let command = resolved[0]
        .interactions
        .on_click
        .clone()
        .expect("dynamic scoped command");
    let mut vm = ScopeRootVm::default();
    command.execute(&mut vm);
    assert_eq!(vm.child.count, 1);
    assert_eq!(vm.other.count, 0);

    show.set(false);
    let resolved = match &tree.root.kind {
        WidgetKind::Container { children, .. } => children[0].resolve(None),
        _ => panic!("root should be a container"),
    };
    let command = resolved[0]
        .interactions
        .on_click
        .clone()
        .expect("dynamic scoped command");
    command.execute(&mut vm);
    assert_eq!(vm.child.count, 1);
    assert_eq!(vm.other.count, 10);
}

#[cfg(feature = "video")]
#[test]
fn video_surface_renders_placeholder_without_frame() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let controller = test_video_controller(crate::video::VideoSurfaceSnapshot {
        intrinsic_size: crate::media::IntrinsicSize::from_pixels(16, 9),
        texture: None,
        loading: true,
        error: None,
    });
    let tree: WidgetTree<()> =
        WidgetTree::new(VideoSurface::new(controller).size(dp(160.0), dp(90.0)));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 90.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered.primitives.textures.is_empty());
    assert!(rendered
        .primitives
        .texts
        .iter()
        .any(|text| text.content.contains("loading video")));
}

#[cfg(feature = "video")]
#[test]
fn video_surface_idle_placeholder_uses_surface_background() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let background = Color::hexa(0x123456FF);
    let radius = dp(12.0);
    let controller = test_video_controller(crate::video::VideoSurfaceSnapshot {
        intrinsic_size: crate::media::IntrinsicSize::ZERO,
        texture: None,
        loading: false,
        error: None,
    });
    let tree: WidgetTree<()> = WidgetTree::new(
        VideoSurface::new(controller)
            .size(dp(160.0), dp(90.0))
            .style(move |mode| {
                let mut style = VideoSurfaceStyle::default_for(mode);
                style.surface.background = Some(background.into());
                style.surface.border_radius = Some(radius.into());
                style
            }),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 90.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered.primitives.textures.is_empty());
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == background && shape.corner_radius == radius.get()));
    assert!(rendered
        .primitives
        .texts
        .iter()
        .any(|text| text.content.contains("video unavailable")));
}

#[cfg(feature = "video")]
#[test]
fn video_surface_renders_texture_when_frame_exists() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let texture = std::sync::Arc::new(crate::media::TextureFrame::new(
        32,
        18,
        vec![255; 32 * 18 * 4],
    ));
    let controller = test_video_controller(crate::video::VideoSurfaceSnapshot {
        intrinsic_size: crate::media::IntrinsicSize::from_pixels(32, 18),
        texture: Some(texture),
        loading: false,
        error: None,
    });
    let tree: WidgetTree<()> = WidgetTree::new(
        VideoSurface::new(controller)
            .width(dp(160.0))
            .aspect_ratio(32.0 / 18.0),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 90.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.textures.len(), 1);
    assert_eq!(rendered.primitives.textures[0].frame.width, 160.0);
    assert_eq!(rendered.primitives.textures[0].frame.height, 90.0);
}

#[test]
fn binding_driven_children_can_switch_component_types() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let show_button = context.state(false);
    let tree = WidgetTree::new(Stack::<()>::new().child(show_button.signal().map(|value| {
        if value {
            vec![super::Element::from(crate::ui::widget::Button::new(
                "toggle button",
            ))]
        } else {
            vec![Element::from(Text::new("toggle text"))]
        }
    })));

    let mut animations = AnimationEngine::default();
    let text_render = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert_eq!(text_render.primitives.shapes.len(), 0);

    show_button.set(true);
    let button_render = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(!button_render.primitives.shapes.is_empty());
}

#[test]
fn button_label_is_horizontally_centered_but_text_is_not() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let text_tree: WidgetTree<()> = WidgetTree::new(
        Text::new("Center")
            .padding(Insets::all(dp(16.0)))
            .size(dp(160.0), dp(48.0)),
    );
    let text_render = text_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 48.0),
        None,
        None,
        None,
        None,
        false,
    );

    let button_tree: WidgetTree<()> =
        WidgetTree::new(crate::ui::widget::Button::new("Center").size(dp(160.0), dp(48.0)));
    let button_render = button_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 48.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(text_render.primitives.texts.len(), 1);
    assert_eq!(button_render.primitives.texts.len(), 1);
    assert!(button_render.primitives.texts[0].frame.x > text_render.primitives.texts[0].frame.x);
}

#[test]
fn disabled_button_exposes_disabled_hit_for_cursor_only() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        crate::ui::widget::Button::new("disabled")
            .disable(true)
            .size(dp(120.0), dp(40.0)),
    );

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        Some(Point::new(dp(10.0), dp(10.0))),
        None,
    );
    assert!(matches!(hit, Some(super::HitInteraction::Disabled { .. })));
}

#[test]
fn button_uses_theme_radius_by_default() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(crate::ui::widget::Button::new("radius").size(dp(120.0), dp(40.0)));
    let default_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState::default(),
        crate::ui::widget::common::ButtonVariantKind::Primary,
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.corner_radius == default_style.radius.get()));
    assert!(!rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.stroke_width > 0.0 && shape.color == default_style.border_color));
}

#[test]
fn primary_button_uses_hover_background_when_hovered() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let button: Element<()> = crate::ui::widget::Button::new("hover")
        .size(dp(120.0), dp(40.0))
        .into();
    let button_id = button.id;
    let tree: WidgetTree<()> = WidgetTree::new(button);
    let mut hovered_state = WidgetStateMap::default();
    hovered_state.set(
        button_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &hovered_state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let hovered_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
        crate::ui::widget::common::ButtonVariantKind::Primary,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.stroke_width == 0.0 && shape.color == hovered_style.background));
}

#[test]
fn primary_button_hover_background_uses_transition() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let button: Element<()> = crate::ui::widget::Button::new("hover")
        .size(dp(120.0), dp(40.0))
        .into();
    let button_id = button.id;
    let tree: WidgetTree<()> = WidgetTree::new(button);
    let mut hovered_state = WidgetStateMap::default();
    hovered_state.set(
        button_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
    );

    let normal = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let start_background = normal
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.stroke_width == 0.0)
        .expect("button should render a filled background")
        .color;

    let hovered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &hovered_state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let immediate_background = hovered
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.stroke_width == 0.0)
        .expect("hovered button should render a filled background")
        .color;

    std::thread::sleep(std::time::Duration::from_millis(100));
    let mid = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &hovered_state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let mid_background = mid
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.stroke_width == 0.0)
        .expect("hovered button should keep a filled background")
        .color;

    std::thread::sleep(std::time::Duration::from_millis(140));
    let settled = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &hovered_state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let settled_background = settled
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.stroke_width == 0.0)
        .expect("hovered button should render a filled background after transition")
        .color;
    let start_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState::default(),
        crate::ui::widget::common::ButtonVariantKind::Primary,
    );
    let hovered_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
        crate::ui::widget::common::ButtonVariantKind::Primary,
    );

    assert_eq!(start_background, start_style.background);
    assert_eq!(immediate_background, start_background);
    assert_ne!(mid_background, start_background);
    assert_ne!(mid_background, hovered_style.background);
    assert_eq!(settled_background, hovered_style.background);
}

#[test]
fn pressed_button_background_takes_priority_over_focus_fill() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let button: Element<()> = crate::ui::widget::Button::new("focus")
        .size(dp(120.0), dp(40.0))
        .into();
    let button_id = button.id;
    let tree: WidgetTree<()> = WidgetTree::new(button);
    let mut state = WidgetStateMap::default();
    state.set(
        button_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            focused: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let pressed_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            focused: true,
            ..Default::default()
        },
        crate::ui::widget::common::ButtonVariantKind::Primary,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.stroke_width == 0.0 && shape.color == pressed_style.background));
}

#[test]
fn focused_secondary_button_keeps_default_border() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let button: Element<()> = crate::ui::widget::Button::new("focus")
        .secondary()
        .size(dp(120.0), dp(40.0))
        .into();
    let button_id = button.id;
    let tree: WidgetTree<()> = WidgetTree::new(button);
    let mut state = WidgetStateMap::default();
    state.set(
        button_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            focused: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let focused_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            focused: true,
            ..Default::default()
        },
        crate::ui::widget::common::ButtonVariantKind::Secondary,
    );
    let default_style = default_button_style(
        &theme,
        Default::default(),
        crate::ui::widget::common::ButtonVariantKind::Secondary,
    );
    let hovered_pressed_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            ..Default::default()
        },
        crate::ui::widget::common::ButtonVariantKind::Secondary,
    );

    assert_eq!(
        focused_style.border_color,
        hovered_pressed_style.border_color
    );

    assert!(
        rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.stroke_width > 0.0
                && shape.color == hovered_pressed_style.border_color)
    );
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.stroke_width == theme.focus_ring.width.get()
            && shape.color == theme.focus_ring.color
            && shape.rect.width > dp(120.0)));
    assert_eq!(default_style.border_color, default_style.border_color);
}

#[test]
fn focused_ghost_button_keeps_default_visuals() {
    let theme = Theme::default();
    let focused_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState {
            focused: true,
            ..Default::default()
        },
        crate::ui::widget::common::ButtonVariantKind::Ghost,
    );
    let default_style = default_button_style(
        &theme,
        Default::default(),
        crate::ui::widget::common::ButtonVariantKind::Ghost,
    );

    assert_eq!(focused_style.background, default_style.background);
    assert_eq!(focused_style.border_color, default_style.border_color);
}

#[test]
fn secondary_button_uses_theme_border_by_default() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        crate::ui::widget::Button::new("secondary")
            .secondary()
            .size(dp(120.0), dp(40.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let default_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState::default(),
        crate::ui::widget::common::ButtonVariantKind::Secondary,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == default_style.border_color
            && shape.stroke_width == default_style.border_width.get()));
}

#[test]
fn danger_button_has_no_default_border() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        crate::ui::widget::Button::new("danger")
            .danger()
            .size(dp(120.0), dp(40.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let default_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState::default(),
        crate::ui::widget::common::ButtonVariantKind::Danger,
    );

    assert!(!rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.stroke_width > 0.0 && shape.color == default_style.border_color));
}

#[test]
fn explicit_button_transparent_border_overrides_theme_border() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        crate::ui::widget::Button::new("border")
            .style(|mode| button_style(mode, None, Some(dp(0.0)), Some(Color::TRANSPARENT)))
            .size(dp(120.0), dp(40.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let default_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState::default(),
        crate::ui::widget::common::ButtonVariantKind::Primary,
    );

    assert!(!rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.stroke_width > 0.0 && shape.color == default_style.border_color));
}

#[test]
fn explicit_button_radius_overrides_theme_radius() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        crate::ui::widget::Button::new("radius")
            .style(|mode| button_style(mode, Some(dp(12.0)), None, None))
            .size(dp(120.0), dp(40.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.corner_radius == 12.0));
}

#[test]
fn switch_renders_custom_track_and_thumb_colors() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let active_background = Color::hexa(0x10B981FF);
    let inactive_background = Color::hexa(0x475569FF);
    let active_thumb = Color::hexa(0xECFDF5FF);
    let tree: WidgetTree<()> = WidgetTree::new(Switch::new(true).size(dp(52.0), dp(30.0)).style(
        move |mode| {
            switch_style(
                mode,
                active_background,
                inactive_background,
                Some(active_thumb),
                None,
            )
        },
    ));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == active_background));
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.color == active_thumb));

    let inactive_tree: WidgetTree<()> = WidgetTree::new(
        Switch::new(false)
            .size(dp(52.0), dp(30.0))
            .style(move |mode| {
                switch_style(
                    mode,
                    active_background,
                    inactive_background,
                    None,
                    Some(Color::WHITE),
                )
            }),
    );
    let inactive_render = inactive_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(inactive_render
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == inactive_background));
}

#[test]
fn switch_uses_theme_defaults_when_styles_are_not_explicitly_set() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Switch::new(false));
    let default_style = default_switch_style(&theme);
    let default_radius = default_style.radius.resolve().get();
    let default_track = super::resolve_stateful_widget_color(
        &default_style.track,
        crate::ui::theme::WidgetState::default(),
    );
    let default_thumb = super::resolve_stateful_widget_color(
        &default_style.thumb,
        crate::ui::theme::WidgetState::default(),
    );
    let default_border = super::resolve_stateful_widget_color(
        &default_style.border,
        crate::ui::theme::WidgetState::default(),
    );
    let default_border_width = default_style.border_width.resolve().get();

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == default_track));
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.color == default_thumb));
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == default_track && shape.corner_radius == default_radius));
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == default_border && shape.stroke_width == default_border_width));

    let checked_tree: WidgetTree<()> = WidgetTree::new(Switch::new(true));
    let checked_track = super::resolve_stateful_widget_color(
        &default_style.track_checked,
        crate::ui::theme::WidgetState::default(),
    );
    let checked_thumb = super::resolve_stateful_widget_color(
        &default_style.thumb_checked,
        crate::ui::theme::WidgetState::default(),
    );
    let checked_rendered = checked_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(checked_rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == checked_track && shape.corner_radius == default_radius));
    assert!(checked_rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.color == checked_thumb));

    let hovered_switch: Element<()> = Switch::new(true).into();
    let hovered_switch_id = hovered_switch.id;
    let hovered_tree: WidgetTree<()> = WidgetTree::new(hovered_switch);
    let mut hovered_state = WidgetStateMap::default();
    hovered_state.set(
        hovered_switch_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
    );
    let hovered_rendered = hovered_tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        None,
        None,
        &hovered_state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let hovered_checked_thumb = super::resolve_stateful_widget_color(
        &default_style.thumb_checked,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
    );
    let hovered_checked_track = super::resolve_stateful_widget_color(
        &default_style.track_checked,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
    );
    assert!(hovered_rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.color == hovered_checked_thumb));
    assert!(hovered_rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == hovered_checked_track));
}

#[test]
fn checked_switch_thumb_uses_white_across_hover_states() {
    let theme = Theme::dark();

    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();

    let tree: WidgetTree<()> = WidgetTree::new(Switch::new(true));
    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.color == Color::WHITE));

    let hovered_switch: Element<()> = Switch::new(true).into();
    let hovered_switch_id = hovered_switch.id;
    let hovered_tree: WidgetTree<()> = WidgetTree::new(hovered_switch);
    let mut hovered_state = WidgetStateMap::default();
    hovered_state.set(
        hovered_switch_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
    );
    let hovered_rendered = hovered_tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        None,
        None,
        &hovered_state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(hovered_rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.color == Color::WHITE));
}

#[test]
fn focused_switch_keeps_pressed_colors_and_renders_focus_ring() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let switch: Element<()> = Switch::new(true).into();
    let switch_id = switch.id;
    let tree: WidgetTree<()> = WidgetTree::new(switch);
    let mut state = WidgetStateMap::default();
    state.set(
        switch_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            focused: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        None,
        None,
        &state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let switch_style = default_switch_style(&theme);
    let base_state = crate::ui::theme::WidgetState {
        hovered: true,
        pressed: true,
        ..Default::default()
    };
    let focused_state = crate::ui::theme::WidgetState {
        hovered: true,
        pressed: true,
        focused: true,
        ..Default::default()
    };

    assert!(rendered.primitives.shapes.iter().any(|shape| shape.color
        == super::resolve_stateful_widget_color(&switch_style.track_checked, base_state)));
    assert_eq!(
        super::resolve_stateful_widget_color(&switch_style.track_checked, focused_state),
        super::resolve_stateful_widget_color(&switch_style.track_checked, base_state)
    );
    assert_eq!(
        super::resolve_stateful_widget_color(&switch_style.border_checked, focused_state),
        super::resolve_stateful_widget_color(&switch_style.border_checked, base_state)
    );
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.stroke_width == theme.focus_ring.width.get()
            && shape.color == theme.focus_ring.color
            && shape.rect.width > dp(42.0)));
}

#[test]
fn button_focus_ring_override_changes_overlay_without_affecting_layout() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let button: Element<()> = crate::ui::widget::Button::new("focus")
        .style(|mode| {
            let mut style = ButtonStyle::default_for(
                mode,
                crate::ui::widget::common::ButtonVariantKind::Primary,
            );
            style.focus_ring = Some(crate::ui::widget::FocusRingOverride {
                color: Some(Color::hexa(0x22C55EFF)),
                width: Some(dp(3.0)),
                gap: Some(dp(4.0)),
                enabled: Some(true),
            });
            style
        })
        .size(dp(120.0), dp(40.0))
        .into();
    let button_id = button.id;
    let tree: WidgetTree<()> = WidgetTree::new(button);
    let mut state = WidgetStateMap::default();
    state.set(
        button_id,
        crate::ui::theme::WidgetState {
            focused: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.stroke_width == 3.0
            && shape.color == Color::hexa(0x22C55EFF)
            && shape.rect.width > dp(120.0)
            && shape.rect.height > dp(40.0)));
}

#[test]
fn focus_ring_overlay_is_not_clipped() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let button: Element<()> = crate::ui::widget::Button::new("focus")
        .size(dp(120.0), dp(40.0))
        .into();
    let button_id = button.id;
    let tree: WidgetTree<()> = WidgetTree::new(button);
    let mut state = WidgetStateMap::default();
    state.set(
        button_id,
        crate::ui::theme::WidgetState {
            focused: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    let ring = rendered
        .primitives
        .overlay_shapes
        .iter()
        .find(|shape| shape.stroke_width == theme.focus_ring.width.get())
        .expect("focused button should render focus ring overlay");
    assert_eq!(ring.clip_rect, None);
    assert_eq!(ring.clip_mask, None);
}

#[test]
fn neutral_components_remain_transparent_by_default() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let tree: WidgetTree<()> = WidgetTree::new(
        Stack::new()
            .size(dp(120.0), dp(80.0))
            .child(Image::from_bytes(ONE_BY_ONE_GIF).size(dp(40.0), dp(40.0)))
            .child(Canvas::new(Vec::<CanvasItem>::new()).size(dp(40.0), dp(20.0))),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .all(|shape| shape.color.a == 0));
}

#[test]
fn switch_thumb_animates_between_positions() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let checked = context.state(false);
    let tree: WidgetTree<()> = WidgetTree::new(Switch::new(checked.signal().animated(
        crate::animation::Transition::ease_in_out(std::time::Duration::from_millis(180)),
    )));

    let mut animations = AnimationEngine::default();
    let initial = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 60.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let start_x = initial.primitives.overlay_shapes[0].rect.x;

    checked.set(true);
    let toggled = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 60.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let immediate_x = toggled.primitives.overlay_shapes[0].rect.x;

    std::thread::sleep(std::time::Duration::from_millis(100));
    let mid = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 60.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let mid_x = mid.primitives.overlay_shapes[0].rect.x;

    std::thread::sleep(std::time::Duration::from_millis(140));
    let end = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 60.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let end_x = end.primitives.overlay_shapes[0].rect.x;

    assert_eq!(immediate_x, start_x);
    assert!(mid_x > start_x);
    assert!(mid_x < end_x);
}

#[test]
fn selectable_text_renders_selection_highlight() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let text: Element<()> = Text::new("hello").user_select(true).into();
    let text_id = text.id;
    let tree = WidgetTree::new(text);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 40.0),
        None,
        None,
        Some(text_id),
        Some(&TextEditState {
            cursor: 5,
            anchor: 1,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|primitive| { primitive.color == theme.colors.selection.with_alpha_factor(1.0) }));
}

#[test]
fn textarea_renders_multiline_caret_on_second_line() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let text: Element<()> = Textarea::new("hello\nworld").height(dp(120.0)).into();
    let text_id = text.id;
    let tree = WidgetTree::new(text);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 120.0),
        Some(text_id),
        Some(&TextEditState {
            cursor: "hello\nwo".len(),
            anchor: "hello\nwo".len(),
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        Some(text_id),
        Some(&TextEditState::caret_at("hello\nworld")),
        true,
    );

    let caret = rendered
        .primitives
        .overlay_shapes
        .last()
        .expect("caret should be rendered");
    assert!(caret.rect.y > dp(20.0));
}

#[test]
fn textarea_uses_scroll_offset_when_unfocused() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let text: Element<()> = Textarea::new("line 0\nline 1\nline 2\nline 3")
        .height(dp(52.0))
        .into();
    let text_id = text.id;
    let tree = WidgetTree::new(text);
    let viewport = Rect::new(0.0, 0.0, 220.0, 52.0);

    let baseline = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );

    let mut scroll_offsets = HashMap::new();
    scroll_offsets.insert(text_id, Point::new(Dp::ZERO, dp(18.0)));
    let scrolled = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &scroll_offsets,
        viewport,
        None,
        None,
        None,
        None,
        false,
    );

    let baseline_text = baseline
        .primitives
        .texts
        .iter()
        .find(|primitive| primitive.content.contains("line 0"))
        .expect("baseline textarea text should render");
    let scrolled_text = scrolled
        .primitives
        .texts
        .iter()
        .find(|primitive| primitive.content.contains("line 0"))
        .expect("scrolled textarea text should render");

    assert!(scrolled_text.frame.y < baseline_text.frame.y);
}

#[test]
fn textarea_only_emits_visible_text_primitives_for_large_content() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let content = (0..100)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let tree: WidgetTree<()> = WidgetTree::new(Textarea::new(content).height(dp(52.0)));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 52.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered.primitives.texts.len() <= 3);
    assert!(rendered
        .primitives
        .texts
        .iter()
        .all(|primitive| !primitive.content.contains("line 50")));
}

#[test]
fn textarea_shows_scrollbar_by_default() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Textarea::new("line 0\nline 1\nline 2\nline 3\nline 4\nline 5").height(dp(52.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 52.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(!rendered.scroll_regions.is_empty());
    assert!(rendered
        .scroll_regions
        .iter()
        .any(|region| region.vertical_thumb.is_some()));
    assert!(!rendered.primitives.overlay_shapes.is_empty());
}

#[test]
fn textarea_keeps_wrapped_text_and_caret_clear_of_vertical_scrollbar() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let content = "W".repeat(240);
    let text: Element<()> = Textarea::new(content.clone())
        .size(dp(220.0), dp(52.0))
        .auto_wrap(true)
        .into();
    let text_id = text.id;
    let tree = WidgetTree::new(text);
    let viewport = Rect::new(0.0, 0.0, 220.0, 52.0);

    let baseline = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );

    let baseline_region = baseline
        .scroll_regions
        .iter()
        .find(|region| region.id == text_id)
        .expect("textarea should register a scroll region");
    let style = TextareaStyle::default_for(infer_theme_mode(&theme));
    let text = super::text_with_typography(content.clone(), &style.text_style);
    let (font_size, line_height, letter_spacing) =
        resolved_text_metrics(&text, &theme, UnitContext::default());
    let request = TextFontRequest {
        preferred_font: text.font_family.as_deref().or(theme
            .typography
            .body
            .font_family
            .as_deref()),
        weight: text.font_weight.unwrap_or(theme.typography.body.weight),
    };
    let layout = font_manager.measure_text_layout_wrapped(
        &content,
        request,
        font_size,
        line_height,
        letter_spacing,
        crate::ui::widget::text_input_layout_width(
            baseline_region.content_viewport,
            true,
            true,
            super::CARET_WIDTH,
        ),
    );
    let cursor = layout.line_end(0);

    let focused = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        Some(text_id),
        Some(&TextEditState {
            cursor,
            anchor: cursor,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        Some(text_id),
        Some(&TextEditState::caret_at(&content)),
        true,
    );

    let scroll_region = focused
        .scroll_regions
        .iter()
        .find(|region| region.id == text_id)
        .expect("textarea should register a scroll region");
    let vertical_track = scroll_region
        .vertical_track
        .expect("textarea should show a vertical scrollbar");
    let max_right = vertical_track.x + dp(0.1);

    assert!(focused
        .primitives
        .texts
        .iter()
        .all(|primitive| primitive.frame.right() <= max_right));

    let caret = focused
        .primitives
        .overlay_shapes
        .iter()
        .find(|primitive| (primitive.rect.width.get() - super::CARET_WIDTH).abs() <= 0.01)
        .expect("caret should be rendered");
    assert!(
        caret.rect.right() <= max_right,
        "caret_right={} track_x={} viewport_right={}",
        caret.rect.right().get(),
        vertical_track.x.get(),
        scroll_region.content_viewport.right().get(),
    );
}

#[test]
fn textarea_can_hide_scrollbar() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Textarea::new("line 0\nline 1\nline 2\nline 3\nline 4\nline 5")
            .height(dp(52.0))
            .show_scrollbar(false),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 52.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .scroll_regions
        .iter()
        .any(|region| region.vertical_thumb.is_some() || region.horizontal_thumb.is_some()));
    assert!(rendered.primitives.overlay_shapes.is_empty());
}

#[test]
fn textarea_auto_wrap_false_enables_horizontal_scroll_region() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let long_line = "0123456789abcdef0123456789abcdef0123456789abcdef";
    let tree: WidgetTree<()> = WidgetTree::new(
        Textarea::new(long_line)
            .size(dp(120.0), dp(60.0))
            .auto_wrap(false),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 60.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .scroll_regions
        .iter()
        .any(|region| region.overflow_x == Overflow::Scroll));
}

#[test]
fn input_renders_composition_preview_text() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let text: Element<()> = Input::new("abc").into();
    let text_id = text.id;
    let tree = WidgetTree::new(text);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 60.0),
        Some(text_id),
        Some(&TextEditState {
            cursor: 2,
            anchor: 2,
            composition: Some(crate::ui::widget::CompositionState {
                replace_range: (1, 2),
                text: "XYZ".to_string(),
                cursor: Some((0, 2)),
            }),
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        Some(text_id),
        Some(&TextEditState::caret_at("abc")),
        true,
    );

    assert!(rendered
        .primitives
        .texts
        .iter()
        .any(|primitive| primitive.content == "aXYZc"));
}

#[test]
fn input_uses_custom_selection_and_caret_colors() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let selection = Color::hexa(0x11AA33FF);
    let caret = Color::hexa(0xCC2211FF);
    let tree: WidgetTree<()> = WidgetTree::new(Input::new("hello").style(move |mode| {
        let mut style = InputStyle::default_for(mode);
        style.selection = Some(selection.into());
        style.caret = Some(caret.into());
        style
    }));
    let text_id = tree.root.id;

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 60.0),
        Some(text_id),
        Some(&TextEditState {
            cursor: 4,
            anchor: 1,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        Some(text_id),
        Some(&TextEditState::caret_at("hello")),
        true,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|primitive| primitive.color == selection.with_alpha_factor(1.0)));
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|primitive| primitive.color == caret.with_alpha_factor(1.0)));
}

#[test]
fn single_line_input_scroll_clips_text_to_inner_content_rect() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Input::new("0123456789abcdef0123456789").size(dp(96.0), dp(40.0)));
    let text_id = tree.root.id;

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 96.0, 40.0),
        Some(text_id),
        Some(&TextEditState {
            cursor: "0123456789abcdef0123456789".len(),
            anchor: "0123456789abcdef0123456789".len(),
            composition: None,
            scroll_x: dp(80.0),
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        Some(text_id),
        Some(&TextEditState::caret_at("0123456789abcdef0123456789")),
        true,
    );

    let text = rendered
        .primitives
        .texts
        .last()
        .expect("input text should be rendered");
    let expected_clip = Rect::new(12.0, 8.0, 72.0, 24.0);
    assert_eq!(text.clip_rect, Some(expected_clip));
    assert!(text.frame.x < expected_clip.x);
    assert!(text.frame.width > expected_clip.width);
}
