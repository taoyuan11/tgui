use super::*;

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
fn background_shadow_is_emitted_before_blur_and_fill() {
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
                Some(test_shadow()),
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

    assert!(matches!(
        rendered.primitives.commands.get(0),
        Some(crate::ui::widget::RenderCommand::Texture(_))
    ));
    assert!(matches!(
        rendered.primitives.commands.get(1),
        Some(crate::ui::widget::RenderCommand::BackdropBlur(_))
    ));
    assert!(matches!(
        rendered.primitives.commands.get(2),
        Some(crate::ui::widget::RenderCommand::Shape(_))
    ));
}

#[test]
fn background_shadow_does_not_expand_hit_region() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let element: Element<()> = Stack::new()
        .size(dp(120.0), dp(80.0))
        .style(|mode| {
            container_style(
                mode,
                Some(Color::hexa(0x112233AA)),
                None,
                None,
                None,
                Some(test_shadow()),
                None,
                None,
                None,
            )
        })
        .on_click(Command::new(|_: &mut ()| {}))
        .into();
    let widget_id = element.id;
    let tree = WidgetTree::new(element);

    let computed = tree.compute_scene(
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

    let region = computed
        .hit_regions
        .iter()
        .find(|region| matches!(region.interaction, super::HitInteraction::Widget { id, .. } if id == widget_id))
        .expect("widget hit region should exist");
    assert_eq!(region.rect, Rect::new(0.0, 0.0, 120.0, 80.0));
}

#[test]
fn background_shadow_negative_spread_can_skip_render_without_panicking() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(40.0), dp(24.0)).style(|mode| {
            let mut shadow = test_shadow();
            shadow.spread = dp(-40.0);
            container_style(
                mode,
                Some(Color::hexa(0x112233AA)),
                None,
                None,
                None,
                Some(shadow),
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
        Rect::new(0.0, 0.0, 40.0, 24.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .commands
        .iter()
        .all(|command| { !matches!(command, crate::ui::widget::RenderCommand::Texture(_)) }));
}

#[test]
fn background_shadow_texture_size_matches_primitive_frame_with_positive_offset() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(100.0), dp(100.0)).style(|mode| {
            container_style(
                mode,
                Some(Color::hexa(0xFFFFFFFF)),
                None,
                None,
                None,
                Some(Shadow {
                    offset_x: Dp::ZERO,
                    offset_y: dp(7.0),
                    blur: dp(30.0),
                    spread: Dp::ZERO,
                    color: Color::hexa(0x64646F52),
                }),
                None,
                Some(dp(50.0)),
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
        Rect::new(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    let shadow_texture = rendered
        .primitives
        .commands
        .iter()
        .find_map(|command| match command {
            crate::ui::widget::RenderCommand::Texture(texture) => Some(texture),
            _ => None,
        })
        .expect("shadow texture should be emitted");

    let expected_width = shadow_texture.frame.width.get().ceil().max(1.0) as u32;
    let expected_height = shadow_texture.frame.height.get().ceil().max(1.0) as u32;
    assert_eq!(
        shadow_texture.texture.size(),
        (expected_width, expected_height)
    );
}

#[test]
fn background_shadow_reuses_cached_texture_when_widget_moves() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(100.0), dp(100.0)).style(|mode| {
            container_style(
                mode,
                Some(Color::hexa(0xFFFFFFFF)),
                None,
                None,
                None,
                Some(Shadow {
                    offset_x: Dp::ZERO,
                    offset_y: dp(7.0),
                    blur: dp(30.0),
                    spread: Dp::ZERO,
                    color: Color::hexa(0x64646F52),
                }),
                None,
                Some(dp(50.0)),
                None,
            )
        }));

    let first = tree.render_output(
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
    let second = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 120.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    let first_texture = first
        .primitives
        .commands
        .iter()
        .find_map(|command| match command {
            crate::ui::widget::RenderCommand::Texture(texture) => Some(texture),
            _ => None,
        })
        .expect("shadow texture should be emitted");
    let second_texture = second
        .primitives
        .commands
        .iter()
        .find_map(|command| match command {
            crate::ui::widget::RenderCommand::Texture(texture) => Some(texture),
            _ => None,
        })
        .expect("shadow texture should be emitted after moving");

    assert!(Arc::ptr_eq(&first_texture.texture, &second_texture.texture));
}

