use crate::foundation::color::Color;
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::Value;
use crate::ui::theme::{FontWeight, Stateful, TextStyle};
use crate::ui::unit::sp;

const HOVER_LIGHTEN: f32 = 0.1;
const SURFACE_HOVER_LIGHTEN: f32 = 0.06;
const BORDER_HOVER_LIGHTEN: f32 = 0.12;
const SCROLLBAR_HOVER_LIGHTEN: f32 = 0.18;

#[derive(Clone)]
pub(super) struct Palette {
    pub(super) primary: Color,
    pub(super) on_primary: Color,
    pub(super) error: Color,
    pub(super) on_error: Color,
    pub(super) surface: Color,
    pub(super) surface_low: Color,
    pub(super) surface_high: Color,
    pub(super) on_surface: Color,
    pub(super) on_surface_muted: Color,
    pub(super) outline: Color,
    pub(super) outline_muted: Color,
    pub(super) disabled_surface: Color,
    pub(super) disabled_content: Color,
    pub(super) text_primary: Color,
    pub(super) scrollbar_track: Color,
    pub(super) scrollbar_thumb: Stateful<Color>,
    pub(super) switch_track: Color,
}

pub(super) fn palette(mode: ResolvedThemeMode) -> Palette {
    match mode {
        ResolvedThemeMode::Light => Palette {
            primary: Color::hexa(0x2563EBFF),
            on_primary: Color::WHITE,
            error: Color::hexa(0xDC2626FF),
            on_error: Color::WHITE,
            surface: Color::hexa(0xFFFFFFFF),
            surface_low: Color::hexa(0xFAFAFAFF),
            surface_high: Color::hexa(0xF0F0F0FF),
            on_surface: Color::hexa(0x262626FF),
            on_surface_muted: Color::hexa(0x737373FF),
            outline: Color::hexa(0xD9D9D9FF),
            outline_muted: Color::hexa(0xEDEDEDFF),
            disabled_surface: Color::hexa(0xF0F0F0FF),
            disabled_content: Color::hexa(0xBFBFBFFF),
            text_primary: Color::BLACK,
            scrollbar_track: Color::hexa(0xF5F5F5FF),
            scrollbar_thumb: stateful_colors(
                Color::hexa(0xBFBFBFB8),
                Color::hexa(0xBFBFBFB8).lighten(SCROLLBAR_HOVER_LIGHTEN),
                Color::hexa(0xBFBFBFB8).darken(SCROLLBAR_HOVER_LIGHTEN),
                Color::hexa(0xF0F0F0FF),
            )
            .map(|value| value.resolve()),
            switch_track: Color::hexa(0xBFBFBFFF),
        },
        ResolvedThemeMode::Dark => Palette {
            primary: Color::hexa(0x3B82F6FF),
            on_primary: Color::WHITE,
            error: Color::hexa(0xEF4444FF),
            on_error: Color::WHITE,
            surface: Color::hexa(0x1F1F1FFF),
            surface_low: Color::hexa(0x262626FF),
            surface_high: Color::hexa(0x303030FF),
            on_surface: Color::hexa(0xF5F5F5FF),
            on_surface_muted: Color::hexa(0xA6A6A6FF),
            outline: Color::hexa(0x424242FF),
            outline_muted: Color::hexa(0x595959FF),
            disabled_surface: Color::hexa(0x303030FF),
            disabled_content: Color::hexa(0x8C8C8CFF),
            text_primary: Color::WHITE,
            scrollbar_track: Color::hexa(0x1F1F1FFF),
            scrollbar_thumb: stateful_colors(
                Color::hexa(0x595959B8),
                Color::hexa(0x595959B8).lighten(SCROLLBAR_HOVER_LIGHTEN),
                Color::hexa(0x595959B8).darken(SCROLLBAR_HOVER_LIGHTEN),
                Color::hexa(0x303030FF),
            )
            .map(|value| value.resolve()),
            switch_track: Color::hexa(0x595959FF),
        },
    }
}

pub(super) fn body_text_style() -> TextStyle {
    TextStyle {
        font_family: None,
        size: sp(16.0),
        line_height: Some(sp(22.0)),
        weight: FontWeight::Regular,
        letter_spacing: Some(sp(0.0)),
    }
}

pub(super) fn label_text_style() -> TextStyle {
    TextStyle {
        font_family: None,
        size: sp(14.0),
        line_height: Some(sp(18.0)),
        weight: FontWeight::Medium,
        letter_spacing: Some(sp(0.0)),
    }
}

pub(super) fn hover_lighten() -> f32 {
    HOVER_LIGHTEN
}

pub(super) fn surface_hover_lighten() -> f32 {
    SURFACE_HOVER_LIGHTEN
}

pub(super) fn border_hover_lighten() -> f32 {
    BORDER_HOVER_LIGHTEN
}

pub(super) fn stateful_single(
    normal: Color,
    hovered: Color,
    pressed: Color,
    disabled: Color,
) -> Stateful<Value<Color>> {
    stateful_colors(normal, hovered, pressed, disabled)
}

pub(super) fn stateful_colors(
    normal: Color,
    hovered: Color,
    pressed: Color,
    disabled: Color,
) -> Stateful<Value<Color>> {
    Stateful {
        normal: Value::Static(normal),
        hovered: Value::Static(hovered),
        pressed: Value::Static(pressed),
        disabled: Value::Static(disabled),
    }
}

trait MapStateful<T> {
    fn map<U>(self, mapper: impl Fn(T) -> U) -> Stateful<U>;
}

impl<T> MapStateful<T> for Stateful<T> {
    fn map<U>(self, mapper: impl Fn(T) -> U) -> Stateful<U> {
        Stateful {
            normal: mapper(self.normal),
            hovered: mapper(self.hovered),
            pressed: mapper(self.pressed),
            disabled: mapper(self.disabled),
        }
    }
}
