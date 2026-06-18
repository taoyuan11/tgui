use super::*;

#[test]
fn background_image_produces_texture_primitive() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(64.0), dp(64.0)).style_full(|ctx| {
            container_style(
                ctx,
                None,
                None,
                Some(BackgroundImage::from_bytes(ONE_BY_ONE_GIF)),
                None,
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
    let tree: WidgetTree<()> = WidgetTree::new(Stack::new().size(dp(80.0), dp(50.0)).style_full(
        move |ctx| {
            container_style(
                ctx,
                Some(fallback),
                None,
                Some(BackgroundImage::new(MediaSource::bytes(
                    b"not-an-image".as_slice(),
                ))),
                None,
                None,
                None,
                None,
                None,
            )
        },
    ));

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
        WidgetTree::new(Stack::new().size(dp(96.0), dp(72.0)).style_full(|ctx| {
            container_style(
                ctx,
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
                None,
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
        WidgetTree::new(Stack::new().size(dp(64.0), dp(64.0)).style_full(|ctx| {
            container_style(
                ctx,
                None,
                None,
                Some(BackgroundImage::from_bytes(ONE_BY_ONE_GIF)),
                None,
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
