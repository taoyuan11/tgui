use super::{
    measure_node, push_textarea_primitives, resolved_text_metrics, MeasureContext, Rect,
    ScenePrimitives, ScrollRegionSource, Text, TextAreaRenderResult, WidgetId,
};
use crate::animation::AnimationEngine;
use crate::foundation::binding::InvalidationSignal;
use crate::media::MediaManager;
use crate::ui::layout::Insets;
use crate::ui::theme::Theme;
use crate::ui::unit::UnitContext;
use crate::ui::widget::InputEditState;
use std::time::Instant;
use taffy::Size as TaffySize;

fn font_manager() -> crate::text::font::FontManager {
    crate::text::font::FontManager::new(&crate::text::font::FontCatalog::default())
}

#[test]
fn textarea_measure_respects_min_rows_and_max_rows() {
    let font_manager = font_manager();
    let theme = Theme::dark();
    let media = MediaManager::new(InvalidationSignal::new());
    let units = UnitContext::new(1.0, 1.0);
    let textarea_style = theme.components.textarea.resolve(Default::default());

    let short_text = Text::new("short");
    let (_, line_height, _) = resolved_text_metrics(&short_text, &theme, units);
    let expected_min_height = (line_height * 3.0 + units.resolve_dp(textarea_style.padding_y) * 2.0)
        .max(units.resolve_dp(textarea_style.min_height));

    let mut short_context = MeasureContext::TextArea {
        text: short_text,
        placeholder: Text::new(""),
        rows: 2,
        min_rows: Some(3),
        max_rows: Some(6),
    };
    let short_size = measure_node(
        Some(&mut short_context),
        TaffySize {
            width: Some(180.0),
            height: None,
        },
        &font_manager,
        &theme,
        &media,
        units,
    );
    assert!((short_size.height - expected_min_height).abs() < 0.5);

    let mut tall_context = MeasureContext::TextArea {
        text: Text::new(
            "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron",
        ),
        placeholder: Text::new(""),
        rows: 2,
        min_rows: Some(3),
        max_rows: Some(4),
    };
    let tall_size = measure_node(
        Some(&mut tall_context),
        TaffySize {
            width: Some(120.0),
            height: None,
        },
        &font_manager,
        &theme,
        &media,
        units,
    );
    let expected_max_height = (line_height * 4.0 + units.resolve_dp(textarea_style.padding_y) * 2.0)
        .max(units.resolve_dp(textarea_style.min_height));
    assert!((tall_size.height - expected_max_height).abs() < 0.5);
}

#[test]
fn textarea_render_adds_internal_scroll_region_when_content_overflows() {
    let font_manager = font_manager();
    let theme = Theme::dark();
    let units = UnitContext::new(1.0, 1.0);
    let textarea_style = theme.components.textarea.resolve(Default::default());
    let padding = Insets::symmetric(textarea_style.padding_x, textarea_style.padding_y);
    let widget_id = WidgetId::next();
    let mut animations = AnimationEngine::default();
    let mut scene = ScenePrimitives::default();
    let text = Text::new(
        "first line\nsecond line\nthird line\nfourth line\nfifth line\nsixth line",
    );

    let render: TextAreaRenderResult = push_textarea_primitives(
        Rect::new(0.0, 0.0, 180.0, 84.0),
        &text,
        &Text::new(""),
        &text.content.resolve(),
        &font_manager,
        &theme,
        units,
        &mut animations,
        Instant::now(),
        &mut scene,
        padding,
        &textarea_style,
        1.0,
        widget_id,
        Some(&InputEditState::default()),
        true,
        None,
        None,
        3,
        None,
        Some(3),
        None,
        None,
    );

    let scroll_region = render
        .scroll_region
        .expect("overflowing textarea should expose an internal scroll region");
    assert_eq!(scroll_region.source, ScrollRegionSource::Input { widget_id });
    assert!(scroll_region.vertical_track.is_some());
    assert!(scroll_region.vertical_thumb.is_some());
    assert!(render.ime_cursor_area.is_some());
}
