use crate::foundation::window_control::WindowResizeDirection;
use crate::platform::cursor::CursorIcon;
use crate::platform::event::{MouseButton, MouseScrollDelta};
use crate::platform::keyboard::ModifiersState;
use crate::platform::window::ResizeDirection;
use crate::text::font::{FontManager, TextFontRequest, TextLayoutInfo};
use crate::ui::theme::Theme;
use crate::ui::unit::{sp, Sp, UnitContext};
use crate::ui::widget::{
    text_input_content_geometry, text_input_content_viewport, text_input_layout_width,
    CanvasMouseButton, CursorStyle, Point, Rect, Text, TextInputContentGeometry,
};

use super::input;

pub(super) fn is_primary_shortcut_modifier(modifiers: ModifiersState) -> bool {
    #[cfg(target_os = "macos")]
    {
        modifiers.super_key()
    }

    #[cfg(not(target_os = "macos"))]
    {
        modifiers.control_key()
    }
}

pub(super) fn mouse_scroll_delta(delta: MouseScrollDelta) -> Point {
    const LINE_SCROLL_STEP: f32 = 40.0;

    match delta {
        MouseScrollDelta::LineDelta(x, y) => Point::new(x * LINE_SCROLL_STEP, y * LINE_SCROLL_STEP),
        MouseScrollDelta::PixelDelta(position) => Point::new(position.x as f32, position.y as f32),
    }
}

pub(super) fn canvas_mouse_button(button: Option<MouseButton>) -> Option<CanvasMouseButton> {
    match button? {
        MouseButton::Left => Some(CanvasMouseButton::Left),
        MouseButton::Right => Some(CanvasMouseButton::Right),
        MouseButton::Middle => Some(CanvasMouseButton::Middle),
        MouseButton::Back => Some(CanvasMouseButton::Back),
        MouseButton::Forward => Some(CanvasMouseButton::Forward),
        other => Some(CanvasMouseButton::Other(other as u16)),
    }
}

pub(super) fn cursor_icon(cursor_style: CursorStyle) -> CursorIcon {
    match cursor_style {
        CursorStyle::Default => CursorIcon::Default,
        CursorStyle::Pointer => CursorIcon::Pointer,
        CursorStyle::Text => CursorIcon::Text,
        CursorStyle::Crosshair => CursorIcon::Crosshair,
        CursorStyle::Move => CursorIcon::Move,
        CursorStyle::NotAllowed => CursorIcon::NotAllowed,
        CursorStyle::Grab => CursorIcon::Grab,
        CursorStyle::Grabbing => CursorIcon::Grabbing,
        CursorStyle::EwResize => CursorIcon::EwResize,
        CursorStyle::NsResize => CursorIcon::NsResize,
        CursorStyle::NeswResize => CursorIcon::NeswResize,
        CursorStyle::NwseResize => CursorIcon::NwseResize,
    }
}

impl From<WindowResizeDirection> for ResizeDirection {
    fn from(direction: WindowResizeDirection) -> Self {
        match direction {
            WindowResizeDirection::East => Self::East,
            WindowResizeDirection::North => Self::North,
            WindowResizeDirection::NorthEast => Self::NorthEast,
            WindowResizeDirection::NorthWest => Self::NorthWest,
            WindowResizeDirection::South => Self::South,
            WindowResizeDirection::SouthEast => Self::SouthEast,
            WindowResizeDirection::SouthWest => Self::SouthWest,
            WindowResizeDirection::West => Self::West,
        }
    }
}

pub(super) fn input_text_layout(
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    text_style: &Text,
    current_text: &str,
    multiline: bool,
    auto_wrap: bool,
    wrap_width: f32,
) -> (TextLayoutInfo, f32, f32) {
    let (text_request, font_size, line_height, letter_spacing) =
        resolved_input_text_metrics(theme, units, text_style);
    let layout = if multiline && auto_wrap {
        font_manager.measure_text_layout_wrapped(
            current_text,
            text_request,
            font_size,
            line_height,
            letter_spacing,
            wrap_width,
        )
    } else {
        font_manager.measure_text_layout(
            current_text,
            text_request,
            font_size,
            line_height,
            letter_spacing,
        )
    };
    (layout, font_size, line_height)
}

pub(super) fn resolved_input_text_metrics<'a>(
    theme: &'a Theme,
    units: UnitContext,
    text_style: &'a Text,
) -> (TextFontRequest<'a>, f32, f32, f32) {
    let default_style = &theme.typography.body;
    let default_size = default_style.size.max(sp(1.0));
    let resolved_font_size = text_style.font_size.unwrap_or(default_size);
    let font_size = units.resolve_sp(resolved_font_size);
    let default_line_height_sp = text_style
        .line_height
        .or(default_style.line_height)
        .unwrap_or(resolved_font_size * 1.25);
    let default_line_height = units.resolve_sp(default_line_height_sp);
    let default_font_size = units.resolve_sp(default_size);
    let scaled_line_height = if default_font_size > 0.0 {
        default_line_height * (font_size / default_font_size)
    } else {
        default_line_height
    };
    let line_height = default_line_height
        .max(scaled_line_height)
        .max(font_size + 4.0);
    let letter_spacing = units.resolve_sp(
        text_style
            .letter_spacing
            .unwrap_or(default_style.letter_spacing.unwrap_or(Sp::ZERO)),
    );
    let text_request = TextFontRequest {
        preferred_font: text_style
            .font_family
            .as_deref()
            .or(default_style.font_family.as_deref()),
        weight: text_style.font_weight.unwrap_or(default_style.weight),
    };
    (text_request, font_size, line_height, letter_spacing)
}

pub(super) fn text_cursor_index_at_point(
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    frame: Rect,
    padding: crate::ui::layout::Insets,
    text_style: &Text,
    current_text: &str,
    multiline: bool,
    auto_wrap: bool,
    show_scrollbar: bool,
    scroll: Point,
    point: Point,
) -> usize {
    if current_text.is_empty() {
        return 0;
    }

    let content_viewport =
        text_input_content_viewport(frame, padding, multiline, show_scrollbar, theme, units);
    let (layout, _font_size, line_height) = input_text_layout(
        font_manager,
        theme,
        units,
        text_style,
        current_text,
        multiline,
        auto_wrap,
        text_input_layout_width(
            content_viewport,
            multiline,
            auto_wrap,
            input::INPUT_CARET_WIDTH,
        ),
    );
    text_cursor_index_from_layout_at_point(
        &layout,
        line_height,
        content_viewport,
        multiline,
        auto_wrap,
        scroll,
        point,
    )
}

pub(super) fn text_cursor_index_from_layout_at_point(
    layout: &TextLayoutInfo,
    line_height: f32,
    content_viewport: Rect,
    multiline: bool,
    auto_wrap: bool,
    scroll: Point,
    point: Point,
) -> usize {
    let TextInputContentGeometry { content_frame, .. } = text_input_content_geometry(
        layout,
        line_height,
        content_viewport,
        multiline,
        auto_wrap,
        scroll,
        input::INPUT_CARET_WIDTH,
    );
    let local_x = (point.x - content_frame.x).max(0.0);
    if multiline {
        let local_y = (point.y - content_frame.y).max(0.0);
        layout.index_for_point(local_x.get(), local_y.get())
    } else {
        layout.index_for_x(local_x.get())
    }
}
