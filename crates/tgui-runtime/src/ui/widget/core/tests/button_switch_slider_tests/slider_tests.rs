use super::*;

#[test]
fn slider_renders_track_fill_thumb_ticks_and_value_label() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Slider::new(50.0, 0.0, 100.0)
            .width(dp(220.0))
            .show_ticks(true)
            .show_value_label(true),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 48.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered.primitives.shapes.len() >= 4);
    assert!(rendered.primitives.overlay_shapes.is_empty());
    assert!(!rendered.primitives.texts.is_empty());
}

#[test]
fn slider_thumb_shadow_renders_before_thumb_fill_without_changing_hit_geometry() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let element: Element<()> = Slider::new(50.0, 0.0, 100.0)
        .width(dp(220.0))
        .style_full(|ctx| {
            let mut style = SliderStyle::default_for_theme(ctx.theme);
            style.thumb_shadow = Some(test_shadow());
            style
        })
        .into();
    let widget_id = element.id;
    let tree = WidgetTree::new(element);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 48.0),
        None,
        None,
        None,
        None,
        false,
    );
    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 48.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(!rendered.primitives.textures.is_empty());
    let thumb_fill = rendered
        .primitives
        .shapes
        .iter()
        .find(|shape| (shape.rect.width.get() - shape.rect.height.get()).abs() <= 0.01)
        .expect("slider thumb should render as a shape");
    let slider_hit = computed
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            super::HitInteraction::Slider { id, thumb_rect, .. } if *id == widget_id => {
                Some(*thumb_rect)
            }
            _ => None,
        })
        .expect("slider hit region should exist");
    let texture_index = rendered
        .primitives
        .commands
        .iter()
        .position(|command| matches!(command, crate::ui::widget::RenderCommand::Texture(_)))
        .expect("slider thumb shadow should render as a texture command");
    let thumb_fill_index = rendered
        .primitives
        .commands
        .iter()
        .position(|command| match command {
            crate::ui::widget::RenderCommand::Shape(shape) => shape.rect == thumb_fill.rect,
            _ => false,
        })
        .expect("slider thumb fill should render as a shape command");
    assert!(texture_index < thumb_fill_index);
    assert_eq!(thumb_fill.rect, slider_hit);
}

#[test]
fn slider_default_uses_token_track_without_thumb_outline() {
    let theme = Theme::light();
    let style = SliderStyle::default_for_theme(&theme);
    assert_eq!(style.track.normal.resolve(), theme.colors.surface_high);
    assert_eq!(style.active_track.normal.resolve(), theme.colors.primary);
    assert_eq!(style.border_width.resolve(), theme.border.none);

    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Slider::new(50.0, 0.0, 100.0).width(dp(220.0)));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 48.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(!rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.stroke_width > 0.0));
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == theme.colors.surface_high));
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == theme.colors.primary));
}

#[test]
fn vertical_slider_renders_bottom_up_fill_and_hit_geometry() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let track = Color::hexa(0x334155FF);
    let active_track = Color::hexa(0x22C55EFF);
    let thumb = Color::hexa(0xF97316FF);
    let element: Element<()> = Slider::new(75.0, 0.0, 100.0)
        .vertical()
        .size(dp(48.0), dp(220.0))
        .style_full(move |ctx| {
            let mut style = SliderStyle::default_for_theme(ctx.theme);
            style.track = stateful(track.into());
            style.active_track = stateful(active_track.into());
            style.thumb = stateful(thumb.into());
            style.thumb_shadow = None;
            style
        })
        .into();
    let widget_id = element.id;
    let tree = WidgetTree::new(element);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 64.0, 240.0),
        None,
        None,
        None,
        None,
        false,
    );
    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 64.0, 240.0),
        None,
        None,
        None,
        None,
        false,
    );

    let (track_rect, thumb_rect) = computed
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            super::HitInteraction::Slider {
                id,
                track_rect,
                thumb_rect,
                ..
            } if *id == widget_id => Some((*track_rect, *thumb_rect)),
            _ => None,
        })
        .expect("slider hit region should exist");
    assert!(track_rect.height > track_rect.width);

    let thumb_center_y = thumb_rect.y + thumb_rect.height * 0.5;
    let normalized_from_bottom =
        ((track_rect.bottom() - thumb_center_y).get() / track_rect.height.get()).clamp(0.0, 1.0);
    assert!((normalized_from_bottom - 0.75).abs() <= 0.01);

    let active_shape = rendered
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.color == active_track)
        .expect("vertical slider should render active fill");
    let expected_active_height = track_rect.height.get() * 0.75;
    assert!((active_shape.rect.height.get() - expected_active_height).abs() <= 0.01);
    assert!(
        (active_shape.rect.y.get() - (track_rect.bottom().get() - expected_active_height)).abs()
            <= 0.01
    );
    assert_eq!(active_shape.rect.width, track_rect.width);
}

#[test]
fn slider_renders_custom_colors() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let track = Color::hexa(0x334155FF);
    let active_track = Color::hexa(0x22C55EFF);
    let thumb = Color::hexa(0xF97316FF);
    let tick = Color::hexa(0x38BDF8FF);
    let label = Color::hexa(0xEAB308FF);
    let tree: WidgetTree<()> = WidgetTree::new(
        Slider::new(75.0, 0.0, 100.0)
            .width(dp(220.0))
            .show_ticks(true)
            .show_value_label(true)
            .style_full(move |ctx| {
                let mut style = SliderStyle::default_for_theme(ctx.theme);
                style.track = stateful(track.into());
                style.active_track = stateful(active_track.into());
                style.thumb = stateful(thumb.into());
                style.tick = stateful(tick.into());
                style.label = stateful(label.into());
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
        Rect::new(0.0, 0.0, 240.0, 48.0),
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
        .any(|shape| shape.color == track));
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == active_track));
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == thumb || shape.color == tick));
    assert!(rendered
        .primitives
        .texts
        .iter()
        .any(|text| text.color == label));
}
