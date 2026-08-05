pub(super) use super::*;

use std::time::Duration;

use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::Value;
use crate::ui::widget::Flex;
use crate::ui::widget::{Button, Popover, PopoverStyle, PopoverTriggerMode, RenderPrimitive};

fn popover_scene_at(
    tree: &WidgetTree<()>,
    theme: &Theme,
    font_manager: &FontManager,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    reduced_motion: bool,
    viewport: Rect,
    now: Instant,
) -> crate::ui::widget::ComputedScene<()> {
    tree.compute_scene_with_units_and_widget_state_at(
        font_manager,
        theme,
        media,
        UnitContext::default(),
        animations,
        reduced_motion,
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
        false,
        now,
    )
}

#[test]
fn popover_builder_attaches_descriptor() {
    let element: Element<()> = Popover::new(Button::new("More"))
        .content(Text::new("popover"))
        .into();
    let descriptor = element
        .popover
        .as_ref()
        .expect("popover descriptor attached");
    assert!(!descriptor.is_open());
    assert_eq!(descriptor.trigger_mode, PopoverTriggerMode::Click);
}

#[test]
fn popover_open_false_renders_only_trigger() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(Text::new("popover body"))
            .open(false),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 400.0, 300.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(rendered.primitives.overlay_texts.is_empty());
}

#[test]
fn popover_open_true_emits_overlay_content() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(
                Flex::vertical()
                    .gap(dp(8.0))
                    .child(Text::new("popover body"))
                    .child(Button::new("Action")),
            )
            .open(true),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 480.0, 320.0),
        None,
        None,
        None,
        None,
        false,
    );
    let labels: Vec<_> = rendered
        .primitives
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect();
    assert!(labels.iter().any(|text| *text == "popover body"));
    assert!(labels.iter().any(|text| *text == "Action"));
    assert!(!rendered.primitives.overlay_shapes.is_empty());
}

#[test]
fn popover_focus_order_stays_global_across_page_siblings() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let before: Element<()> = Button::new("Before").into();
    let before_id = before.id;
    let trigger: Element<()> = Button::new("Trigger").into();
    let trigger_id = trigger.id;
    let popover_first: Element<()> = Button::new("Popover first").into();
    let popover_first_id = popover_first.id;
    let popover_second: Element<()> = Button::new("Popover second").into();
    let popover_second_id = popover_second.id;
    let after: Element<()> = Button::new("After").into();
    let after_id = after.id;
    let popover: Element<()> = Popover::new(trigger)
        .content(Flex::vertical().child(popover_first).child(popover_second))
        .open(true)
        .into();
    let tree = WidgetTree::new(Flex::vertical().child(before).child(popover).child(after));

    let computed = popover_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        Rect::new(0.0, 0.0, 640.0, 480.0),
        Instant::now(),
    );
    let mut focus_order = computed
        .hit_regions
        .iter()
        .chain(computed.overlay_hit_regions.iter())
        .filter_map(|region| {
            region
                .focus
                .as_ref()
                .map(|focus| (focus.order, focus.widget_id))
        })
        .collect::<Vec<_>>();
    focus_order.sort_by_key(|(order, _)| *order);

    assert_eq!(
        focus_order,
        vec![
            (0, before_id),
            (1, trigger_id),
            (2, popover_first_id),
            (3, popover_second_id),
            (4, after_id),
        ]
    );
}

#[test]
fn popover_pointer_style_emits_overlay_mesh() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let mut style = PopoverStyle::default_for_theme(&Theme::light());
    style.pointer_size = Some(dp(8.0));

    let tree: WidgetTree<()> = WidgetTree::new(
        Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(Text::new("popover body"))
            .style_full(move |_| style.clone())
            .open(true),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 480.0, 320.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(rendered
        .primitives
        .overlay_texts
        .iter()
        .any(|text| text.content.as_ref() == "popover body"));
    assert!(!rendered.primitives.overlay_meshes.is_empty());
}

#[test]
fn popover_style_defaults_match_expected_baseline() {
    for mut theme in [Theme::light(), Theme::dark()] {
        for density in [
            crate::ui::theme::Density::Compact,
            crate::ui::theme::Density::Comfortable,
            crate::ui::theme::Density::Spacious,
        ] {
            theme.density = density;
            let style = PopoverStyle::default_for_theme(&theme);
            assert_eq!(style.shadow, theme.elevation.md);
            assert_ne!(style.shadow, theme.elevation.lg);
            assert!(style.pointer_size.is_none());
        }
    }

    let theme = Theme::light();
    let comfortable = PopoverStyle::default_for_theme(&theme);
    assert_eq!(comfortable.padding, Insets::all(theme.spacing.md));
    assert_eq!(comfortable.min_width, dp(220.0));
    assert_eq!(comfortable.offset, theme.spacing.sm);
}

#[test]
fn popover_default_shadow_is_lighter_than_explicit_large_elevation() {
    fn shadow_texture(use_large_shadow: bool) -> crate::ui::widget::TexturePrimitive {
        let theme = Theme::light();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let popover = Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(Text::new("popover body"))
            .open(true);
        let tree: WidgetTree<()> = if use_large_shadow {
            WidgetTree::new(popover.style(|style, context| {
                style.shadow = context.theme.elevation.lg.clone();
            }))
        } else {
            WidgetTree::new(popover)
        };
        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 640.0, 480.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert_eq!(rendered.primitives.overlay_textures.len(), 1);
        rendered.primitives.overlay_textures[0].clone()
    }

    let default = shadow_texture(false);
    let large = shadow_texture(true);
    assert!(default.frame.width < large.frame.width);
    assert!(default.frame.height < large.frame.height);
    assert!(default.texture.size().0 < large.texture.size().0);
    assert!(default.texture.size().1 < large.texture.size().1);
}

#[test]
fn popover_scene_tracks_theme_density_without_custom_style() {
    fn render_surface(density: crate::ui::theme::Density) -> (PopoverStyle, RenderPrimitive) {
        let mut theme = Theme::light();
        theme.density = density;
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
                .content(Text::new("popover body"))
                .open(true),
        );
        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 640.0, 480.0),
            None,
            None,
            None,
            None,
            false,
        );
        let style = PopoverStyle::default_for_theme(&theme);
        let surface = rendered
            .primitives
            .overlay_shapes
            .iter()
            .find(|shape| shape.color == style.background.resolve() && shape.stroke_width == 0.0)
            .cloned()
            .expect("open popover should emit its themed surface");
        (style, surface)
    }

    let (compact_style, compact_surface) = render_surface(crate::ui::theme::Density::Compact);
    let (spacious_style, spacious_surface) = render_surface(crate::ui::theme::Density::Spacious);
    assert!(compact_surface.rect.width >= compact_style.min_width);
    assert!(spacious_surface.rect.width >= spacious_style.min_width);
    assert!(spacious_surface.rect.width > compact_surface.rect.width);
    assert!(spacious_surface.rect.height > compact_surface.rect.height);
    assert!(compact_surface.corner_radius > 0.0);
    assert_eq!(
        compact_surface.corner_radius,
        spacious_surface.corner_radius
    );
}

#[test]
fn click_and_hover_preview_exposes_a_controlled_open_request() {
    let element: Element<()> = Popover::new(Button::new("More"))
        .content(Text::new("popover"))
        .open(true)
        .trigger_mode(PopoverTriggerMode::ClickAndHoverPreview)
        .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
        .into();
    assert!(element
        .popover
        .as_ref()
        .and_then(|popover| popover.on_open_change.as_ref())
        .is_some());
}

#[test]
fn popover_close_keeps_visual_fade_but_releases_interaction_immediately() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 640.0, 420.0);
    let context = test_context();
    let open = context.state(false);
    let action: Element<()> = Button::new("Action").into();
    let tree: WidgetTree<()> = WidgetTree::new(
        Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(action)
            .open(open.signal())
            .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {})),
    );
    let mut theme = Theme::light();
    theme.motion.fast_ms = 180;
    let start = Instant::now();
    let mut animations = AnimationEngine::default();

    let _ = popover_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        start,
    );
    open.set(true);
    let open_start = start + Duration::from_millis(1);
    let _ = popover_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        open_start,
    );
    let opened = popover_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        open_start + Duration::from_millis(181),
    );
    assert!(!opened.overlay_hit_regions.is_empty());
    assert!(!opened.overlay_close_handlers.is_empty());

    open.set(false);
    let closing = popover_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        open_start + Duration::from_millis(182),
    );
    assert!(closing
        .scene
        .overlay_texts
        .iter()
        .any(|text| text.content.as_ref() == "Action"));
    assert!(closing.overlay_hit_regions.is_empty());
    assert!(closing.overlay_close_handlers.iter().all(|handler| {
        handler.on_close.is_none()
            && !handler.close_on_outside_click
            && !handler.close_on_escape
            && handler.return_focus_to.is_none()
    }));
    assert!(closing.focus_scopes.is_empty());
    assert!(closing.scroll_regions.is_empty());

    let closed = popover_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        open_start + Duration::from_millis(363),
    );
    assert!(closed.scene.overlay_texts.is_empty());
}

#[test]
fn popover_motion_is_reduced_motion_safe_and_refresh_rate_independent() {
    fn sampled_alpha(frame_interval: Duration, reduced_motion: bool) -> u8 {
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let viewport = Rect::new(0.0, 0.0, 640.0, 420.0);
        let context = test_context();
        let open = context.state(false);
        let surface = Color::rgba(37, 121, 211, 255);
        let tree: WidgetTree<()> = WidgetTree::new(
            Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
                .content(Text::new("Body"))
                .open(open.signal())
                .style(move |style, _| {
                    style.background = Value::Static(surface);
                    style.border = Value::Static(Color::TRANSPARENT);
                    style.border_width = Value::Static(Dp::ZERO);
                }),
        );
        let mut theme = Theme::light();
        theme.motion.fast_ms = 180;
        let start = Instant::now();
        let animation_start = start + Duration::from_millis(1);
        let sample_elapsed = Duration::from_millis(90);
        let mut animations = AnimationEngine::default();
        let _ = popover_scene_at(
            &tree,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            reduced_motion,
            viewport,
            start,
        );
        open.set(true);
        let _ = popover_scene_at(
            &tree,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            reduced_motion,
            viewport,
            animation_start,
        );
        let mut elapsed = frame_interval;
        while elapsed < sample_elapsed {
            let _ = popover_scene_at(
                &tree,
                &theme,
                &font_manager,
                &media,
                &mut animations,
                reduced_motion,
                viewport,
                animation_start + elapsed,
            );
            elapsed += frame_interval;
        }
        let sampled = popover_scene_at(
            &tree,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            reduced_motion,
            viewport,
            animation_start + sample_elapsed,
        );
        sampled
            .scene
            .overlay_shapes
            .iter()
            .find(|shape| shape.color.r == surface.r && shape.color.g == surface.g)
            .expect("popover surface")
            .color
            .a
    }

    let at_120_hz = sampled_alpha(Duration::from_secs_f64(1.0 / 120.0), false);
    let at_144_hz = sampled_alpha(Duration::from_secs_f64(1.0 / 144.0), false);
    assert_eq!(at_120_hz, at_144_hz);
    assert!(at_120_hz > 0 && at_120_hz < 255);
    assert_eq!(
        sampled_alpha(Duration::from_secs_f64(1.0 / 120.0), true),
        255
    );
}
