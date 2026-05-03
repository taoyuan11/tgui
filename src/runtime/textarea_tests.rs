use super::{input_cursor_index_at_point_with_state, input_text_layout};
use crate::text::font::{FontCatalog, FontManager};
use crate::ui::layout::Insets;
use crate::ui::theme::Theme;
use crate::ui::unit::{dp, Dp, UnitContext};
use crate::ui::widget::{input_text_viewport, InputEditState, Point, Rect, Text};

#[test]
fn multiline_cursor_hit_testing_accounts_for_vertical_scroll() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let theme = Theme::dark();
    let units = UnitContext::new(1.0, 1.0);
    let frame = Rect::new(0.0, 0.0, 220.0, 84.0);
    let padding = Insets::all(dp(8.0));
    let text_style = Text::new("");
    let content = "first line\nsecond line\nthird line\nfourth line";
    let inner = frame.inset(padding);
    let (layout, _, line_height) = input_text_layout(
        &font_manager,
        &theme,
        units,
        &text_style,
        content,
        true,
        inner.width.get(),
    );
    let line_start = content.find("fourth").expect("target line should exist");
    let target = line_start + 3;
    let (caret_x, caret_y) = layout.point_for_index(target);
    let content_frame =
        input_text_viewport(inner, layout.width, layout.height, line_height, 0.0, layout.width)
            .frame;
    let scroll_y = Dp::new(line_height * 2.0);
    let point = Point::new(
        content_frame.x + caret_x + dp(1.0),
        content_frame.y + caret_y - scroll_y + dp(line_height * 0.5),
    );
    let state = InputEditState {
        scroll_y,
        ..InputEditState::default()
    };

    let resolved = input_cursor_index_at_point_with_state(
        &font_manager,
        &theme,
        units,
        frame,
        padding,
        &text_style,
        content,
        true,
        Some(&state),
        point,
    );
    assert_eq!(resolved, target);
}
