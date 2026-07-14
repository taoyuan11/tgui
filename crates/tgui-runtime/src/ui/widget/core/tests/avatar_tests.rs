use super::*;

use crate::media::MediaBytes;
use crate::ui::layout::{Length, Value};
use crate::ui::theme::Density;
use crate::ui::widget::{Avatar, ResolvedWidgetKind};

fn avatar_root_size(source: impl Into<Element<()>>, theme: &Theme) -> (Dp, Dp) {
    let tree = WidgetTree::new(source);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let layout = tree.build_scene_layout(
        &font_manager,
        theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 96.0),
    );
    let width = match &layout.resolved_root.layout.width {
        Some(Value::Static(Length::Px(value))) => *value,
        other => panic!("avatar width must resolve to px, got {other:?}"),
    };
    let height = match &layout.resolved_root.layout.height {
        Some(Value::Static(Length::Px(value))) => *value,
        other => panic!("avatar height must resolve to px, got {other:?}"),
    };
    (width, height)
}

#[test]
fn avatar_image_and_initials_share_runtime_density_geometry() {
    for (density, expected) in [
        (Density::Compact, dp(32.0)),
        (Density::Comfortable, dp(40.0)),
        (Density::Spacious, dp(48.0)),
    ] {
        for mut theme in [Theme::light(), Theme::dark()] {
            theme.density = density;
            let initials = avatar_root_size(Avatar::initials("TG"), &theme);
            let image = avatar_root_size(
                Avatar::image(MediaSource::bytes(MediaBytes::from_static(ONE_BY_ONE_GIF))),
                &theme,
            );
            assert_eq!(initials, (expected, expected));
            assert_eq!(image, initials, "image and initials geometry must match");
        }
    }
}

#[test]
fn avatar_explicit_size_overrides_runtime_component_geometry_for_each_source() {
    let mut theme = Theme::dark();
    theme.density = Density::Spacious;
    let initials = avatar_root_size(Avatar::initials("TG").size(dp(72.0), dp(36.0)), &theme);
    let image = avatar_root_size(
        Avatar::image(MediaSource::bytes(MediaBytes::from_static(ONE_BY_ONE_GIF)))
            .size(dp(72.0), dp(36.0)),
        &theme,
    );
    assert_eq!(initials, (dp(72.0), dp(36.0)));
    assert_eq!(image, initials);
}

#[test]
fn avatar_sources_keep_their_native_leaf_kind_without_wrappers() {
    let theme = Theme::light();
    let resolve = |element| {
        let tree: WidgetTree<()> = WidgetTree::new(element);
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        tree.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 96.0),
        )
    };
    let image = resolve(Avatar::image(MediaSource::bytes(MediaBytes::from_static(
        ONE_BY_ONE_GIF,
    ))));
    assert!(matches!(
        image.resolved_root.kind,
        ResolvedWidgetKind::Image { .. }
    ));
    let initials = resolve(Avatar::initials("TG"));
    assert!(matches!(
        initials.resolved_root.kind,
        ResolvedWidgetKind::Container { .. }
    ));
}

#[test]
fn avatar_group_overlap_tracks_runtime_style_on_the_same_tree() {
    let tree: WidgetTree<()> = WidgetTree::new(
        crate::ui::widget::AvatarGroup::new(vec![Avatar::initials("A"), Avatar::initials("B")])
            .style_full(|_| crate::ui::widget::AvatarStyle {
                group_overlap: dp(13.0),
                ..crate::ui::widget::AvatarStyle::default_for_theme(
                    &Theme::light(),
                    crate::ui::widget::AvatarShape::Circle,
                )
            }),
    );
    let mut theme = Theme::dark();
    theme.density = Density::Spacious;
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 96.0),
    );
    let ResolvedWidgetKind::Container { layout, .. } = &layout.resolved_root.kind else {
        panic!("avatar group should be a container");
    };
    assert_eq!(
        layout.gap,
        Value::Static(crate::ui::layout::Length::Px(-dp(13.0)))
    );
}

#[test]
fn image_runtime_geometry_tracks_theme_and_preserves_explicit_axis_override() {
    let image = Image::new(MediaSource::bytes(MediaBytes::from_static(ONE_BY_ONE_GIF)))
        .width(dp(73.0))
        .runtime_layout(|layout, context, _, _| {
            let themed = match context.theme.density {
                Density::Compact => dp(24.0),
                Density::Comfortable => dp(32.0),
                Density::Spacious => dp(48.0),
            };
            if layout.width.is_none() {
                layout.width = Some(Value::Static(Length::Px(themed)));
            }
            if layout.height.is_none() {
                layout.height = Some(Value::Static(Length::Px(themed)));
            }
        });
    let tree: WidgetTree<()> = WidgetTree::new(image);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    for (density, expected_height) in [(Density::Compact, dp(24.0)), (Density::Spacious, dp(48.0))]
    {
        let mut theme = Theme::dark();
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
            Rect::new(0.0, 0.0, 160.0, 96.0),
        );
        assert_eq!(
            layout.resolved_root.layout.width,
            Some(Value::Static(Length::Px(dp(73.0))))
        );
        assert_eq!(
            layout.resolved_root.layout.height,
            Some(Value::Static(Length::Px(expected_height)))
        );
        assert!(matches!(
            layout.resolved_root.kind,
            ResolvedWidgetKind::Image { .. }
        ));
    }
}
