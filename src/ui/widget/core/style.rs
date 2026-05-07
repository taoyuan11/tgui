use crate::foundation::color::Color;
use crate::ui::layout::Value;
use crate::ui::theme::{Theme, WidgetState};
use crate::ui::unit::Dp;

#[cfg(feature = "video")]
use super::super::style::VideoSurfaceStyle as WidgetVideoSurfaceStyle;
use super::super::style::{
    infer_theme_mode, ButtonStyle as WidgetButtonStyle, CheckboxStyle as WidgetCheckboxStyle,
    FocusRingOverride, RadioStyle as WidgetRadioStyle, SelectStyle as WidgetSelectStyle,
    TextWidgetStyle,
};
use super::{Text, VisualStyle};

#[derive(Clone)]
pub(super) struct ResolvedButtonStyle {
    pub(super) background: Color,
    pub(super) border_color: Color,
    pub(super) focus_ring: Option<crate::theme::FocusRingStyle>,
    pub(super) border_width: Dp,
    pub(super) radius: Dp,
    pub(super) padding_x: Dp,
    pub(super) padding_y: Dp,
    pub(super) min_height: Dp,
}

#[derive(Clone)]
pub(super) struct ResolvedCheckboxStyle {
    pub(super) background: Color,
    pub(super) border: Color,
    pub(super) focus_ring: Option<crate::theme::FocusRingStyle>,
    pub(super) checkmark: Color,
    pub(super) label: Color,
    pub(super) border_width: Dp,
    pub(super) radius: Dp,
    pub(super) size: Dp,
    pub(super) label_gap: Dp,
    pub(super) text_style: crate::ui::theme::TextStyle,
}

#[derive(Clone)]
pub(super) struct ResolvedRadioStyle {
    pub(super) background: Color,
    pub(super) border: Color,
    pub(super) focus_ring: Option<crate::theme::FocusRingStyle>,
    pub(super) indicator: Color,
    pub(super) label: Color,
    pub(super) border_width: Dp,
    pub(super) radius: Dp,
    pub(super) size: Dp,
    pub(super) label_gap: Dp,
    pub(super) text_style: crate::ui::theme::TextStyle,
}

#[derive(Clone)]
pub(super) struct ResolvedSelectStyle {
    pub(super) background: Color,
    pub(super) text: Color,
    pub(super) placeholder: Color,
    pub(super) border: Color,
    pub(super) focus_ring: Option<crate::theme::FocusRingStyle>,
    pub(super) arrow: Color,
    pub(super) menu_background: Color,
    pub(super) selected_option_background: Color,
    pub(super) border_width: Dp,
    pub(super) radius: Dp,
    pub(super) padding_x: Dp,
    pub(super) padding_y: Dp,
    pub(super) min_height: Dp,
    pub(super) option_height: Dp,
    pub(super) menu_gap: Dp,
    pub(super) text_style: crate::ui::theme::TextStyle,
}

pub(super) fn resolved_button_style(
    style: Option<&super::super::style::StyleResolver<WidgetButtonStyle>>,
    theme: &Theme,
    variant: crate::ui::widget::common::ButtonVariantKind,
) -> WidgetButtonStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| WidgetButtonStyle::default_for(infer_theme_mode(theme), variant))
}

pub(super) fn resolved_checkbox_style(
    style: Option<&super::super::style::StyleResolver<WidgetCheckboxStyle>>,
    theme: &Theme,
) -> WidgetCheckboxStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| WidgetCheckboxStyle::default_for(infer_theme_mode(theme)))
}

pub(super) fn resolved_radio_style(
    style: Option<&super::super::style::StyleResolver<WidgetRadioStyle>>,
    theme: &Theme,
) -> WidgetRadioStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| WidgetRadioStyle::default_for(infer_theme_mode(theme)))
}

pub(super) fn resolved_switch_style(
    style: Option<&super::super::style::StyleResolver<super::super::style::SwitchStyle>>,
    theme: &Theme,
) -> super::super::style::SwitchStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| super::super::style::SwitchStyle::default_for(infer_theme_mode(theme)))
}

pub(super) fn resolved_select_style(
    style: Option<&super::super::style::StyleResolver<WidgetSelectStyle>>,
    theme: &Theme,
) -> WidgetSelectStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| WidgetSelectStyle::default_for(infer_theme_mode(theme)))
}

pub(super) fn resolved_container_style(
    style: Option<&super::super::style::StyleResolver<super::super::style::ContainerStyle>>,
    theme: &Theme,
) -> super::super::style::ContainerStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| {
            super::super::style::ContainerStyle::default_for(infer_theme_mode(theme))
        })
}

pub(super) fn resolved_image_style(
    style: Option<&super::super::style::StyleResolver<super::super::style::ImageStyle>>,
    theme: &Theme,
) -> super::super::style::ImageStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| super::super::style::ImageStyle::default_for(infer_theme_mode(theme)))
}

pub(super) fn resolved_canvas_style(
    style: Option<&super::super::style::StyleResolver<super::super::style::CanvasStyle>>,
    theme: &Theme,
) -> super::super::style::CanvasStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| super::super::style::CanvasStyle::default_for(infer_theme_mode(theme)))
}

pub(super) fn resolved_text_widget_style(
    style: Option<&super::super::style::StyleResolver<TextWidgetStyle>>,
    theme: &Theme,
) -> TextWidgetStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| TextWidgetStyle::default_for(infer_theme_mode(theme)))
}

#[cfg(feature = "video")]
pub(super) fn resolved_video_surface_style(
    style: Option<&super::super::style::StyleResolver<WidgetVideoSurfaceStyle>>,
    theme: &Theme,
) -> WidgetVideoSurfaceStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| WidgetVideoSurfaceStyle::default_for(infer_theme_mode(theme)))
}

pub(super) fn apply_surface_style(
    background: &mut Option<Value<Color>>,
    visual: &mut VisualStyle,
    surface: &super::super::style::WidgetSurfaceStyle,
) {
    *background = surface.background.clone();
    visual.background_brush = surface.background_brush.clone();
    visual.background_image = surface.background_image.clone();
    visual.background_blur = surface.background_blur.clone();
    visual.border_color = surface.border_color.clone();
    visual.border_radius = surface.border_radius.clone();
    visual.border_width = surface.border_width.clone();
    visual.opacity = surface.opacity.clone();
    visual.offset = surface.offset.clone();
}

pub(super) fn apply_text_widget_style(text: &mut Text, style: &TextWidgetStyle) {
    text.background = style.surface.background.clone();
    text.color = Some(style.color.clone());
    text.font_family = style.typography.font_family.clone();
    text.font_size = Some(style.typography.size);
    text.line_height = style.typography.line_height;
    text.font_weight = Some(style.typography.weight);
    text.letter_spacing = style.typography.letter_spacing;
}

pub(super) fn resolve_stateful_widget_color(
    value: &crate::ui::theme::Stateful<Value<Color>>,
    state: WidgetState,
) -> Color {
    value.resolve(state).resolve()
}

pub(super) fn base_interaction_state(mut state: WidgetState) -> WidgetState {
    state.focused = false;
    state
}

pub(super) fn resolve_focus_ring(
    theme: &Theme,
    override_style: Option<&FocusRingOverride>,
    state: WidgetState,
) -> Option<crate::theme::FocusRingStyle> {
    if state.disabled || !state.focused {
        return None;
    }

    let resolved = override_style
        .map(|style| style.resolve(theme))
        .unwrap_or_else(|| theme.focus_ring.clone());
    if !resolved.enabled || resolved.width <= Dp::ZERO {
        return None;
    }
    Some(resolved)
}

pub(super) fn resolve_button_style(
    style: &WidgetButtonStyle,
    state: WidgetState,
    theme: &Theme,
) -> ResolvedButtonStyle {
    let visual_state = base_interaction_state(state);
    ResolvedButtonStyle {
        background: resolve_stateful_widget_color(&style.background, visual_state),
        border_color: resolve_stateful_widget_color(&style.border, visual_state),
        focus_ring: resolve_focus_ring(theme, style.focus_ring.as_ref(), state),
        border_width: style.border_width.resolve(),
        radius: style.radius.resolve(),
        padding_x: style.padding_x,
        padding_y: style.padding_y,
        min_height: style.min_height,
    }
}

pub(super) fn resolve_checkbox_style(
    style: &WidgetCheckboxStyle,
    state: WidgetState,
    checked: bool,
    theme: &Theme,
) -> ResolvedCheckboxStyle {
    let mut control_state = base_interaction_state(state);
    control_state.selected = checked;
    ResolvedCheckboxStyle {
        background: if checked {
            resolve_stateful_widget_color(&style.background_checked, control_state)
        } else {
            resolve_stateful_widget_color(&style.background, control_state)
        },
        border: if checked {
            resolve_stateful_widget_color(&style.border_checked, control_state)
        } else {
            resolve_stateful_widget_color(&style.border, control_state)
        },
        focus_ring: resolve_focus_ring(theme, style.focus_ring.as_ref(), state),
        checkmark: resolve_stateful_widget_color(&style.checkmark, control_state),
        label: resolve_stateful_widget_color(&style.label, control_state),
        border_width: style.border_width.resolve(),
        radius: style.radius.resolve(),
        size: style.size,
        label_gap: style.label_gap,
        text_style: style.text_style.clone(),
    }
}

pub(super) fn resolve_radio_style(
    style: &WidgetRadioStyle,
    state: WidgetState,
    checked: bool,
    theme: &Theme,
) -> ResolvedRadioStyle {
    let mut control_state = base_interaction_state(state);
    control_state.selected = checked;
    ResolvedRadioStyle {
        background: if checked {
            resolve_stateful_widget_color(&style.background_checked, control_state)
        } else {
            resolve_stateful_widget_color(&style.background, control_state)
        },
        border: if checked {
            resolve_stateful_widget_color(&style.border_checked, control_state)
        } else {
            resolve_stateful_widget_color(&style.border, control_state)
        },
        focus_ring: resolve_focus_ring(theme, style.focus_ring.as_ref(), state),
        indicator: resolve_stateful_widget_color(&style.indicator, control_state),
        label: resolve_stateful_widget_color(&style.label, control_state),
        border_width: style.border_width.resolve(),
        radius: style.radius.resolve(),
        size: style.size,
        label_gap: style.label_gap,
        text_style: style.text_style.clone(),
    }
}

pub(super) fn resolve_select_style(
    style: &WidgetSelectStyle,
    state: WidgetState,
    theme: &Theme,
) -> ResolvedSelectStyle {
    let visual_state = base_interaction_state(state);
    ResolvedSelectStyle {
        background: resolve_stateful_widget_color(&style.background, visual_state),
        text: resolve_stateful_widget_color(&style.text, visual_state),
        placeholder: resolve_stateful_widget_color(&style.placeholder, visual_state),
        border: resolve_stateful_widget_color(&style.border, visual_state),
        focus_ring: resolve_focus_ring(theme, style.focus_ring.as_ref(), state),
        arrow: resolve_stateful_widget_color(&style.arrow, visual_state),
        menu_background: style.menu_background.resolve(),
        selected_option_background: style.selected_option_background.resolve(),
        border_width: style.border_width.resolve(),
        radius: style.radius.resolve(),
        padding_x: style.padding_x,
        padding_y: style.padding_y,
        min_height: style.min_height,
        option_height: style.option_height,
        menu_gap: style.menu_gap,
        text_style: style.text_style.clone(),
    }
}

pub(super) fn default_select_menu_option_color(theme: &Theme, state: WidgetState) -> Color {
    let style = WidgetSelectStyle::default_for(infer_theme_mode(theme));
    resolve_stateful_widget_color(&style.option_background, base_interaction_state(state))
}

pub(super) fn default_select_disabled_text_color(theme: &Theme) -> Color {
    let style = WidgetSelectStyle::default_for(infer_theme_mode(theme));
    let mut state = WidgetState::default();
    state.disabled = true;
    resolve_stateful_widget_color(&style.text, state)
}
