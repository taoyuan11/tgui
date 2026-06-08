pub(super) use super::*;

use crate::foundation::view_model::ValueCommand;
use crate::ui::widget::Flex;
use crate::ui::widget::{Button, Popover, PopoverStyle, PopoverTriggerMode};

#[test]
fn popover_builder_attaches_descriptor() {
    let element: Element<()> = Popover::new(Button::new("More"))
        .content(Text::new("popover"))
        .into();
    let descriptor = element
        .popover
        .as_ref()
        .expect("popover descriptor attached");
    assert!(!descriptor.open.resolve());
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
    let light = PopoverStyle::default_for_theme(&Theme::light());
    assert_eq!(light.padding, Insets::all(dp(12.0)));
    assert_eq!(light.min_width, dp(220.0));
    assert_eq!(light.offset, dp(8.0));
    assert!(light.pointer_size.is_none());
}

#[test]
fn click_and_hover_preview_wraps_trigger_click_when_controlled() {
    let element: Element<()> = Popover::new(Button::new("More"))
        .content(Text::new("popover"))
        .open(true)
        .trigger_mode(PopoverTriggerMode::ClickAndHoverPreview)
        .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
        .into();
    assert!(element.interactions.on_click.is_some());
}
