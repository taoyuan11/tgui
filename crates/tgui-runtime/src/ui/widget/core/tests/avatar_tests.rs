use super::*;

use crate::foundation::view_model::Command;
use crate::media::{ContentFit, MediaBytes};
use crate::ui::layout::{Length, Value};
use crate::ui::theme::Density;
use crate::ui::widget::{
    AccessibilityRole, Avatar, DefaultActivation, Element, HitInteraction, ResolvedWidgetKind,
};

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
fn clickable_avatar_is_a_keyboard_activatable_button_but_static_avatar_is_not() {
    let static_avatar: Element<()> = Avatar::initials("TG").into();
    assert_eq!(static_avatar.visual.accessibility_role, None);
    assert!(static_avatar.interactions.on_click.is_none());

    let clickable: Element<()> = Avatar::initials("TG")
        .on_click(Command::new(|_: &mut ()| {}))
        .into();
    let clickable_id = clickable.id;
    assert_eq!(
        clickable.visual.accessibility_role,
        Some(AccessibilityRole::Button)
    );
    assert_eq!(clickable.focus.focusable, Some(true));
    assert_eq!(clickable.focus.tab_index, Some(0));

    let scene = WidgetTree::new(clickable).compute_scene(
        &FontManager::new(&FontCatalog::default()),
        &Theme::default(),
        &test_media(),
        &mut AnimationEngine::default(),
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 96.0, 96.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(scene.hit_regions.iter().any(|hit| matches!(
        hit.interaction,
        HitInteraction::Widget {
            id,
            default_activation: DefaultActivation::EnterAndSpace,
            ..
        } if id == clickable_id
    )));
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
fn avatar_group_overflow_uses_the_overflow_background_token() {
    let overflow_color = Color::hexa(0xE11D48FF);
    let tree: WidgetTree<()> = WidgetTree::new(
        crate::ui::widget::AvatarGroup::new(vec![Avatar::initials("A"), Avatar::initials("B")])
            .max_visible(1)
            .style_full(move |theme_context| {
                let mut style = crate::ui::widget::AvatarStyle::default_for_theme(
                    theme_context.theme,
                    crate::ui::widget::AvatarShape::Circle,
                );
                style.group_overflow_background = Value::Static(overflow_color);
                style
            }),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let rendered = tree.render_output(
        &font_manager,
        &Theme::light(),
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 96.0),
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
        .any(|shape| shape.color == overflow_color));
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

#[test]
fn image_style_fit_applies_unless_the_builder_explicitly_overrides_it() {
    let resolve_fit = |image: Image| {
        let tree: WidgetTree<()> = WidgetTree::new(image);
        let layout = tree.build_scene_layout(
            &FontManager::new(&FontCatalog::default()),
            &Theme::default(),
            &test_media(),
            &mut AnimationEngine::default(),
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 96.0),
        );
        let ResolvedWidgetKind::Image { image, .. } = layout.resolved_root.kind else {
            panic!("image should retain its native leaf kind");
        };
        image.fit
    };
    let source = || MediaSource::bytes(MediaBytes::from_static(ONE_BY_ONE_GIF));

    assert_eq!(
        resolve_fit(Image::new(source()).style(|style, _| style.fit = ContentFit::Fill)),
        ContentFit::Fill,
    );
    assert_eq!(
        resolve_fit(
            Image::new(source())
                .fit(ContentFit::Cover)
                .style(|style, _| style.fit = ContentFit::Fill),
        ),
        ContentFit::Cover,
    );
}
