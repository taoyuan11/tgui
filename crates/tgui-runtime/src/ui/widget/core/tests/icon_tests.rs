use super::*;

use crate::ui::layout::Value;
use crate::ui::widget::{BuiltinIcon, Icon, IconStyle};

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
fn builtin_icon_layout_size_controls_texture_frame() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Flex::horizontal()
            .gap(dp(8.0))
            .child(Icon::builtin(BuiltinIcon::Bell).size(dp(18.0)))
            .child(Icon::builtin(BuiltinIcon::Bell).size(dp(40.0))),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 56.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.textures.len(), 2);
    assert_eq!(rendered.primitives.textures[0].frame.width, dp(18.0));
    assert_eq!(rendered.primitives.textures[0].frame.height, dp(18.0));
    assert_eq!(rendered.primitives.textures[1].frame.width, dp(40.0));
    assert_eq!(rendered.primitives.textures[1].frame.height, dp(40.0));
}

#[test]
fn builtin_icon_default_size_tracks_theme_density_on_the_same_tree() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> =
        WidgetTree::new(Icon::builtin(BuiltinIcon::Search).style_full(|context| {
            let size = match context.theme.density {
                crate::ui::theme::Density::Compact => dp(12.0),
                crate::ui::theme::Density::Comfortable => dp(18.0),
                crate::ui::theme::Density::Spacious => dp(28.0),
            };
            IconStyle {
                color: Value::Static(context.theme.colors.on_surface),
                size,
            }
        }));

    let mut compact = Theme::default();
    compact.density = crate::ui::theme::Density::Compact;
    let mut spacious = Theme::default();
    spacious.density = crate::ui::theme::Density::Spacious;
    let render = |theme: &Theme| {
        let mut animations = AnimationEngine::default();
        tree.render_output(
            &font_manager,
            theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 96.0, 96.0),
            None,
            None,
            None,
            None,
            false,
        )
        .primitives
        .textures[0]
            .frame
    };
    let compact_frame = render(&compact);
    let spacious_frame = render(&spacious);
    assert_eq!(compact_frame.width, dp(12.0));
    assert_eq!(spacious_frame.width, dp(28.0));
    assert_ne!(compact_frame.width, spacious_frame.width);
}

#[test]
fn public_svg_icon_size_tracks_runtime_style_without_freezing_default_theme() {
    let icon = Icon::svg(ONE_BY_ONE_GIF).style_full(|context| IconStyle {
        color: Value::Static(context.theme.colors.on_surface),
        size: if context.theme.density == crate::ui::theme::Density::Spacious {
            dp(30.0)
        } else {
            dp(14.0)
        },
    });
    let tree: WidgetTree<()> = WidgetTree::new(icon);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    for (density, expected) in [
        (crate::ui::theme::Density::Compact, dp(14.0)),
        (crate::ui::theme::Density::Spacious, dp(30.0)),
    ] {
        let mut theme = Theme::light();
        theme.density = density;
        let mut animations = AnimationEngine::default();
        let layout = tree.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 96.0, 96.0),
        );
        assert_eq!(
            layout.resolved_root.layout.width,
            Some(Value::Static(crate::ui::layout::Length::Px(expected)))
        );
    }
}

#[test]
fn common_builtin_icons_render_as_svg_textures() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let icons = [
        BuiltinIcon::Home,
        BuiltinIcon::Settings,
        BuiltinIcon::Bell,
        BuiltinIcon::Mail,
        BuiltinIcon::Lock,
        BuiltinIcon::Eye,
        BuiltinIcon::Edit,
        BuiltinIcon::Copy,
        BuiltinIcon::Download,
        BuiltinIcon::Upload,
        BuiltinIcon::File,
        BuiltinIcon::Folder,
        BuiltinIcon::Trash,
        BuiltinIcon::RefreshCw,
        BuiltinIcon::ExternalLink,
        BuiltinIcon::Menu,
        BuiltinIcon::Filter,
        BuiltinIcon::SortAsc,
        BuiltinIcon::SortDesc,
        BuiltinIcon::Play,
        BuiltinIcon::Pause,
        BuiltinIcon::VolumeUp,
        BuiltinIcon::VolumeDown,
        BuiltinIcon::VolumeOff,
        BuiltinIcon::Palette,
        BuiltinIcon::MapPin,
        BuiltinIcon::Link,
        BuiltinIcon::Heart,
    ];
    let mut row = Flex::horizontal().gap(dp(4.0));
    for icon in icons {
        row = row.child(Icon::builtin(icon));
    }
    let tree: WidgetTree<()> = WidgetTree::new(row);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 960.0, 48.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.textures.len(), icons.len());
    assert!(
        rendered.primitives.texts.is_empty(),
        "common builtin icons should not render icon names as text"
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
