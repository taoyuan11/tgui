use std::time::Duration;

use crate::animation::Transition;
use crate::foundation::color::Color;
use crate::ui::layout::Value;
use crate::ui::theme::{Shadow, StyleContext, Theme, WidgetState};
use crate::ui::unit::Dp;

#[cfg(feature = "video")]
use super::super::style::VideoSurfaceStyle as WidgetVideoSurfaceStyle;
use super::super::style::{
    ButtonStyle as WidgetButtonStyle, CheckboxStyle as WidgetCheckboxStyle,
    DividerStyle as WidgetDividerStyle, FocusRingOverride, InputStyle as WidgetInputStyle,
    ProgressBarStyle as WidgetProgressBarStyle, RadioStyle as WidgetRadioStyle,
    SelectStyle as WidgetSelectStyle, SliderStyle as WidgetSliderStyle,
    SpinnerStyle as WidgetSpinnerStyle, TextWidgetStyle, TextareaStyle as WidgetTextareaStyle,
};
use super::{Text, VisualStyle};

#[derive(Clone)]
#[allow(dead_code)]
pub(super) struct ResolvedButtonStyle {
    pub(super) background: Color,
    pub(super) border_color: Color,
    pub(super) background_value: Value<Color>,
    pub(super) border_color_value: Value<Color>,
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

#[derive(Clone)]
pub(super) struct ResolvedSliderStyle {
    pub(super) track: Color,
    pub(super) active_track: Color,
    pub(super) thumb: Color,
    pub(super) thumb_shadow: Option<Shadow>,
    pub(super) tick: Color,
    pub(super) label: Color,
    pub(super) focus_ring: Option<crate::theme::FocusRingStyle>,
    pub(super) track_height: Dp,
    pub(super) thumb_size: Dp,
    pub(super) radius: Dp,
    pub(super) border_width: Dp,
    pub(super) tick_size: Dp,
    pub(super) label_gap: Dp,
    pub(super) min_width: Dp,
    pub(super) min_height: Dp,
    pub(super) text_style: crate::ui::theme::TextStyle,
}

#[derive(Clone)]
pub(super) struct ResolvedInputStyle {
    pub(super) background: Color,
    pub(super) text: Color,
    pub(super) placeholder: Color,
    pub(super) border: Color,
    pub(super) selection: Option<Color>,
    pub(super) caret: Option<Color>,
    pub(super) border_width: Dp,
    pub(super) radius: Dp,
    pub(super) padding_x: Dp,
    pub(super) padding_y: Dp,
    pub(super) text_style: crate::ui::theme::TextStyle,
}

pub(super) fn apply_local_style<T: Clone>(
    style: Option<&super::super::style::StyleResolver<T>>,
    base: T,
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> T {
    style
        .map(|resolver| resolver.resolve_with(base.clone(), context, style_sheet, visual))
        .unwrap_or(base)
}

pub(super) fn apply_local_style_with_state<T: Clone>(
    style: Option<&super::super::style::StyleResolver<T>>,
    base: T,
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> T {
    style
        .map(|resolver| {
            resolver.resolve_with_state(base.clone(), context, style_sheet, visual, state)
        })
        .unwrap_or(base)
}

#[cfg(test)]
pub(super) fn resolved_button_style(
    style: Option<&super::super::style::StyleResolver<WidgetButtonStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
    variant: crate::ui::widget::common::ButtonVariantKind,
) -> WidgetButtonStyle {
    let base = button_style_base(context, style_sheet, visual, variant);
    apply_local_style(style, base, context, style_sheet, visual)
}

pub(super) fn button_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
    variant: crate::ui::widget::common::ButtonVariantKind,
) -> WidgetButtonStyle {
    let mut base = WidgetButtonStyle::default_for_theme(context.theme, variant);
    context.theme.components.button.apply(&mut base, context);
    style_sheet.apply_button(&mut base, context, variant, visual);
    base
}

pub(super) fn checkbox_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> WidgetCheckboxStyle {
    let mut base = WidgetCheckboxStyle::default_for_theme(context.theme);
    context.theme.components.checkbox.apply(&mut base, context);
    style_sheet.apply_checkbox(&mut base, context, visual);
    base
}

pub(super) fn radio_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> WidgetRadioStyle {
    let mut base = WidgetRadioStyle::default_for_theme(context.theme);
    context.theme.components.radio.apply(&mut base, context);
    style_sheet.apply_radio(&mut base, context, visual);
    base
}

pub(super) fn switch_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> super::super::style::SwitchStyle {
    let mut base = super::super::style::SwitchStyle::default_for_theme(context.theme);
    context.theme.components.switch.apply(&mut base, context);
    style_sheet.apply_switch(&mut base, context, visual);
    base
}

pub(super) fn select_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> WidgetSelectStyle {
    let mut base = WidgetSelectStyle::default_for_theme(context.theme);
    context.theme.components.select.apply(&mut base, context);
    style_sheet.apply_select(&mut base, context, visual);
    base
}

pub(super) fn slider_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> WidgetSliderStyle {
    let mut base = WidgetSliderStyle::default_for_theme(context.theme);
    context.theme.components.slider.apply(&mut base, context);
    style_sheet.apply_slider(&mut base, context, visual);
    base
}

pub(super) fn progress_bar_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> WidgetProgressBarStyle {
    let mut base = WidgetProgressBarStyle::default_for_theme(context.theme);
    context
        .theme
        .components
        .progress_bar
        .apply(&mut base, context);
    style_sheet.apply_progress_bar(&mut base, context, visual);
    base
}

pub(super) fn spinner_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> WidgetSpinnerStyle {
    let mut base = WidgetSpinnerStyle::default_for_theme(context.theme);
    context.theme.components.spinner.apply(&mut base, context);
    style_sheet.apply_spinner(&mut base, context, visual);
    base
}

pub(super) fn divider_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> WidgetDividerStyle {
    let mut base = WidgetDividerStyle::default_for_theme(context.theme);
    context.theme.components.divider.apply(&mut base, context);
    style_sheet.apply_divider(&mut base, context, visual);
    base
}

pub(super) fn input_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> WidgetInputStyle {
    let mut base = WidgetInputStyle::default_for_theme(context.theme);
    context.theme.components.input.apply(&mut base, context);
    style_sheet.apply_input(&mut base, context, visual);
    base
}

pub(super) fn textarea_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> WidgetTextareaStyle {
    let mut base = WidgetTextareaStyle::default_for_theme(context.theme);
    context.theme.components.textarea.apply(&mut base, context);
    style_sheet.apply_textarea(&mut base, context, visual);
    base
}

pub(super) fn input_style_from_textarea_style(style: WidgetTextareaStyle) -> WidgetInputStyle {
    WidgetInputStyle {
        surface: style.surface,
        background: style.background,
        text: style.text,
        placeholder: style.placeholder,
        border: style.border,
        selection: style.selection,
        caret: style.caret,
        border_width: style.border_width,
        radius: style.radius,
        padding_x: style.padding_x,
        padding_y: style.padding_y,
        min_height: style.min_height,
        text_style: style.text_style,
    }
}

pub(super) fn container_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> super::super::style::ContainerStyle {
    let mut base = super::super::style::ContainerStyle::default_for_theme(context.theme);
    context.theme.components.container.apply(&mut base, context);
    style_sheet.apply_container(&mut base, context, visual);
    base
}

pub(super) fn image_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> super::super::style::ImageStyle {
    let mut base = super::super::style::ImageStyle::default_for_theme(context.theme);
    context.theme.components.image.apply(&mut base, context);
    style_sheet.apply_image(&mut base, context, visual);
    base
}

pub(super) fn canvas_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> super::super::style::CanvasStyle {
    let mut base = super::super::style::CanvasStyle::default_for_theme(context.theme);
    context.theme.components.canvas.apply(&mut base, context);
    style_sheet.apply_canvas(&mut base, context, visual);
    base
}

pub(super) fn text_widget_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> TextWidgetStyle {
    let mut base = TextWidgetStyle::default_for_theme(context.theme);
    context.theme.components.text.apply(&mut base, context);
    style_sheet.apply_text(&mut base, context, visual);
    base
}

#[cfg(feature = "video")]
pub(super) fn video_surface_style_base(
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    visual: &VisualStyle,
) -> WidgetVideoSurfaceStyle {
    let mut base = WidgetVideoSurfaceStyle::default_for_theme(context.theme);
    context
        .theme
        .components
        .video_surface
        .apply(&mut base, context);
    style_sheet.apply_video_surface(&mut base, context, visual);
    base
}

pub(super) fn apply_surface_style(
    background: &mut Option<Value<Color>>,
    visual: &mut VisualStyle,
    surface: &super::super::style::WidgetSurfaceStyle,
) {
    super::super::style::merge_surface_style(background, visual, surface);
}

fn merge_runtime_surface<T>(
    runtime_style: &super::ResolvedRuntimeSurfaceStyle<T>,
    style: &T,
) -> (Option<Value<Color>>, VisualStyle)
where
    T: super::ResolvedSurfaceStyle,
{
    let mut background = runtime_style.explicit_background.clone();
    let mut visual = runtime_style.explicit_visual.clone();
    apply_surface_style(&mut background, &mut visual, style.surface());
    (background, visual)
}

pub(super) fn resolved_runtime_text_surface(
    runtime_style: &super::ResolvedRuntimeSurfaceStyle<TextWidgetStyle>,
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    state: WidgetState,
) -> (Option<Value<Color>>, VisualStyle) {
    let mut style = runtime_style.base.clone();
    style_sheet.apply_text_state(&mut style, context, &runtime_style.explicit_visual, state);
    let style = apply_local_style_with_state(
        runtime_style.local.as_ref(),
        style,
        context,
        style_sheet,
        &runtime_style.explicit_visual,
        state,
    );
    merge_runtime_surface(runtime_style, &style)
}

pub(super) fn resolved_runtime_container_surface(
    runtime_style: &super::ResolvedRuntimeSurfaceStyle<super::super::style::ContainerStyle>,
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    state: WidgetState,
) -> (Option<Value<Color>>, VisualStyle) {
    let mut style = runtime_style.base.clone();
    style_sheet.apply_container_state(&mut style, context, &runtime_style.explicit_visual, state);
    let style = apply_local_style_with_state(
        runtime_style.local.as_ref(),
        style,
        context,
        style_sheet,
        &runtime_style.explicit_visual,
        state,
    );
    merge_runtime_surface(runtime_style, &style)
}

pub(super) fn resolved_runtime_image_surface(
    runtime_style: &super::ResolvedRuntimeSurfaceStyle<super::super::style::ImageStyle>,
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    state: WidgetState,
) -> (Option<Value<Color>>, VisualStyle) {
    let mut style = runtime_style.base.clone();
    style_sheet.apply_image_state(&mut style, context, &runtime_style.explicit_visual, state);
    let style = apply_local_style_with_state(
        runtime_style.local.as_ref(),
        style,
        context,
        style_sheet,
        &runtime_style.explicit_visual,
        state,
    );
    merge_runtime_surface(runtime_style, &style)
}

pub(super) fn resolved_runtime_canvas_surface(
    runtime_style: &super::ResolvedRuntimeSurfaceStyle<super::super::style::CanvasStyle>,
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    state: WidgetState,
) -> (Option<Value<Color>>, VisualStyle) {
    let mut style = runtime_style.base.clone();
    style_sheet.apply_canvas_state(&mut style, context, &runtime_style.explicit_visual, state);
    let style = apply_local_style_with_state(
        runtime_style.local.as_ref(),
        style,
        context,
        style_sheet,
        &runtime_style.explicit_visual,
        state,
    );
    merge_runtime_surface(runtime_style, &style)
}

#[cfg(feature = "video")]
pub(super) fn resolved_runtime_video_surface(
    runtime_style: &super::ResolvedRuntimeSurfaceStyle<WidgetVideoSurfaceStyle>,
    context: &StyleContext<'_>,
    style_sheet: &crate::ui::widget::StyleSheet,
    state: WidgetState,
) -> (Option<Value<Color>>, VisualStyle) {
    let mut style = runtime_style.base.clone();
    style_sheet.apply_video_surface_state(
        &mut style,
        context,
        &runtime_style.explicit_visual,
        state,
    );
    let style = apply_local_style_with_state(
        runtime_style.local.as_ref(),
        style,
        context,
        style_sheet,
        &runtime_style.explicit_visual,
        state,
    );
    merge_runtime_surface(runtime_style, &style)
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
    value: &crate::ui::theme::StateValue<Value<Color>>,
    state: WidgetState,
) -> Color {
    value.resolve(state).resolve()
}

pub(super) fn default_state_transition(theme: &Theme, reduced_motion: bool) -> Option<Transition> {
    if reduced_motion {
        None
    } else {
        Some(Transition::ease_out(Duration::from_millis(
            theme.motion.fast_ms,
        )))
    }
}

pub(super) fn default_motion_transition(theme: &Theme, reduced_motion: bool) -> Option<Transition> {
    if reduced_motion {
        None
    } else {
        Some(Transition::ease_in_out(Duration::from_millis(
            theme.motion.normal_ms,
        )))
    }
}

pub(super) fn base_interaction_state(mut state: WidgetState) -> WidgetState {
    state.focused = false;
    state.focus_visible = false;
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
    let background_value = style.background.resolve(visual_state);
    let border_color_value = style.border.resolve(visual_state);
    ResolvedButtonStyle {
        background: background_value.resolve_untracked(),
        border_color: border_color_value.resolve_untracked(),
        background_value,
        border_color_value,
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
    control_state.checked = checked;
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
    control_state.checked = checked;
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

pub(super) fn resolve_input_style(
    style: &WidgetInputStyle,
    state: WidgetState,
) -> ResolvedInputStyle {
    ResolvedInputStyle {
        background: resolve_stateful_widget_color(&style.background, state),
        text: resolve_stateful_widget_color(&style.text, state),
        placeholder: resolve_stateful_widget_color(&style.placeholder, state),
        border: resolve_stateful_widget_color(&style.border, state),
        selection: style.selection.as_ref().map(Value::resolve),
        caret: style.caret.as_ref().map(Value::resolve),
        border_width: style.border_width.resolve(),
        radius: style.radius.resolve(),
        padding_x: style.padding_x,
        padding_y: style.padding_y,
        text_style: style.text_style.clone(),
    }
}

pub(super) fn resolve_slider_style(
    style: &WidgetSliderStyle,
    state: WidgetState,
    theme: &Theme,
) -> ResolvedSliderStyle {
    let visual_state = base_interaction_state(state);
    ResolvedSliderStyle {
        track: resolve_stateful_widget_color(&style.track, visual_state),
        active_track: resolve_stateful_widget_color(&style.active_track, visual_state),
        thumb: resolve_stateful_widget_color(&style.thumb, visual_state),
        thumb_shadow: style.thumb_shadow.clone(),
        tick: resolve_stateful_widget_color(&style.tick, visual_state),
        label: resolve_stateful_widget_color(&style.label, visual_state),
        focus_ring: resolve_focus_ring(theme, style.focus_ring.as_ref(), state),
        track_height: style.track_height,
        thumb_size: style.thumb_size,
        radius: style.radius.resolve(),
        border_width: style.border_width.resolve(),
        tick_size: style.tick_size,
        label_gap: style.label_gap,
        min_width: style.min_width,
        min_height: style.min_height,
        text_style: style.text_style.clone(),
    }
}

pub(super) fn default_select_menu_option_color(theme: &Theme, state: WidgetState) -> Color {
    let style = WidgetSelectStyle::default_for_theme(theme);
    resolve_stateful_widget_color(&style.option_background, base_interaction_state(state))
}

pub(super) fn default_select_disabled_text_color(theme: &Theme) -> Color {
    let style = WidgetSelectStyle::default_for_theme(theme);
    let state = WidgetState {
        disabled: true,
        ..Default::default()
    };
    resolve_stateful_widget_color(&style.text, state)
}
