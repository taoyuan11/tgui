use crate::foundation::color::Color;
use crate::ui::layout::Value;
use crate::ui::theme::{StateValue, Theme};

const HOVER_LIGHTEN: f32 = 0.1;
const SURFACE_HOVER_LIGHTEN: f32 = 0.06;
const BORDER_HOVER_LIGHTEN: f32 = 0.12;
const SCROLLBAR_HOVER_LIGHTEN: f32 = 0.18;

#[derive(Clone)]
pub(crate) struct Palette {
    pub(crate) primary: Color,
    pub(crate) on_primary: Color,
    pub(crate) error: Color,
    pub(crate) on_error: Color,
    pub(crate) surface: Color,
    pub(crate) surface_low: Color,
    pub(crate) surface_high: Color,
    pub(crate) on_surface: Color,
    pub(crate) on_surface_muted: Color,
    pub(crate) outline: Color,
    pub(crate) outline_muted: Color,
    pub(crate) disabled_surface: Color,
    pub(crate) disabled_content: Color,
    pub(crate) text_primary: Color,
    pub(crate) scrollbar_track: Color,
    pub(crate) scrollbar_thumb: StateValue<Color>,
    pub(crate) switch_track: Color,
}

pub(crate) fn palette_from_theme(theme: &Theme) -> Palette {
    let colors = &theme.colors;
    Palette {
        primary: colors.primary,
        on_primary: colors.on_primary,
        error: colors.error,
        on_error: colors.on_error,
        surface: colors.surface,
        surface_low: colors.surface_low,
        surface_high: colors.surface_high,
        on_surface: colors.on_surface,
        on_surface_muted: colors.on_surface_muted,
        outline: colors.outline,
        outline_muted: colors.outline_muted,
        disabled_surface: colors.disabled,
        disabled_content: colors.on_disabled,
        text_primary: colors.on_surface,
        scrollbar_track: colors.surface_low,
        scrollbar_thumb: stateful_colors(
            colors.outline.with_alpha_factor(0.72),
            colors.outline.lighten(SCROLLBAR_HOVER_LIGHTEN),
            colors.outline.darken(SCROLLBAR_HOVER_LIGHTEN),
            colors.disabled,
        )
        .map(|value| value.resolve()),
        switch_track: colors.outline,
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
) -> StateValue<Value<Color>> {
    stateful_colors(normal, hovered, pressed, disabled)
}

pub(super) fn stateful_colors(
    normal: Color,
    hovered: Color,
    pressed: Color,
    disabled: Color,
) -> StateValue<Value<Color>> {
    StateValue::interactive(
        Value::Static(normal),
        Value::Static(hovered),
        Value::Static(pressed),
        Value::Static(disabled),
    )
}

trait MapStateValue<T> {
    fn map<U>(self, mapper: impl Fn(T) -> U) -> StateValue<U>;
}

impl<T> MapStateValue<T> for StateValue<T> {
    fn map<U>(self, mapper: impl Fn(T) -> U) -> StateValue<U> {
        StateValue {
            normal: mapper(self.normal),
            hovered: mapper(self.hovered),
            pressed: mapper(self.pressed),
            disabled: mapper(self.disabled),
            focused: self.focused.map(&mapper),
            focus_visible: self.focus_visible.map(&mapper),
            selected: self.selected.map(&mapper),
            checked: self.checked.map(&mapper),
            open: self.open.map(&mapper),
            invalid: self.invalid.map(mapper),
        }
    }
}
