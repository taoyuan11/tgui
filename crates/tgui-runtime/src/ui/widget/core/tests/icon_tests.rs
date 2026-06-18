use super::*;

use crate::ui::widget::{BuiltinIcon, Icon};

#[test]
fn builtin_icon_renders_as_svg_texture_without_material_text() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Flex::horizontal()
            .gap(dp(8.0))
            .child(Icon::builtin(BuiltinIcon::Search))
            .child(Icon::builtin(BuiltinIcon::Search).style(|style, _| {
                style.color = Color::hexa(0xCC3366FF).into();
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
        Rect::new(0.0, 0.0, 96.0, 48.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(
        rendered.primitives.textures.len(),
        2,
        "default and custom-color builtin icons should render as SVG textures"
    );
    assert!(
        rendered
            .primitives
            .texts
            .iter()
            .all(|text| text.content.as_ref() != "search"),
        "builtin icons should not emit Material icon name text"
    );
}

#[test]
#[allow(deprecated)]
fn named_icon_renders_as_plain_text_without_icon_texture() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Icon::named("search"));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 48.0),
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
        .any(|text| text.content.as_ref() == "search"));
}
