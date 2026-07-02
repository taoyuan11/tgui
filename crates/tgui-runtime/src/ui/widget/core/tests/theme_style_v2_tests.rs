use super::*;

use crate::ui::layout::Value;
use crate::ui::theme::{ComponentThemes, WidgetState};
use crate::ui::widget::common::{ButtonVariantKind, VisualStyle};
use crate::ui::widget::{
    ButtonSelector, DataGridStyle, ListStyle, MenuStyle, ModalStyle, ProgressBarStyle, SelectStyle,
    SliderStyle, StyleSheet, TabsStyle, TreeStyle, WidgetSurfaceStyle,
};

fn normal_color(value: &StateValue<Value<Color>>) -> Color {
    value.normal.clone().resolve()
}

#[test]
fn theme_tokens_drive_default_component_styles() {
    let mut theme = Theme::light();
    theme.colors.primary = Color::hexa(0x2468ACFF);
    theme.colors.on_primary = Color::hexa(0xFEFEFEFF);
    theme.colors.surface = Color::hexa(0x102030FF);
    theme.colors.surface_overlay = Color::hexa(0x102030F5);
    theme.colors.surface_low = Color::hexa(0x203040FF);
    theme.colors.surface_high = Color::hexa(0x304050FF);
    theme.colors.on_surface = Color::hexa(0xE6EDF5FF);
    theme.colors.on_surface_muted = Color::hexa(0xA8B4C2FF);
    theme.colors.error = Color::hexa(0xD13030FF);
    theme.colors.selection = Color::hexa(0x2468AC66);
    theme.colors.outline = Color::hexa(0x607080FF);
    theme.colors.outline_muted = Color::hexa(0x506070AA);
    theme.typography.body.size = sp(17.0);
    theme.typography.label.size = sp(15.0);
    theme.spacing.xs = dp(5.0);
    theme.spacing.sm = dp(9.0);
    theme.spacing.md = dp(21.0);
    theme.spacing.lg = dp(29.0);
    theme.spacing.xl = dp(37.0);
    theme.radius.md = dp(11.0);
    theme.radius.lg = dp(17.0);

    let button = ButtonStyle::default_for_theme(&theme, ButtonVariantKind::Primary);
    assert_eq!(normal_color(&button.background), theme.colors.primary);
    assert_eq!(normal_color(&button.foreground), theme.colors.on_primary);
    assert_eq!(button.radius.resolve(), theme.radius.md);
    assert_eq!(button.padding_x, theme.spacing.sm);
    assert_eq!(button.text_style.size, theme.typography.label.size);

    let input = InputStyle::default_for_theme(&theme);
    assert_eq!(normal_color(&input.background), theme.colors.surface);
    assert_eq!(
        input.caret.as_ref().unwrap().resolve(),
        theme.colors.primary
    );
    assert_eq!(
        input.selection.as_ref().unwrap().resolve(),
        theme.colors.selection
    );
    assert_eq!(input.radius.resolve(), theme.radius.md);
    assert_eq!(input.padding_x, theme.spacing.md - theme.spacing.xs);
    assert_eq!(input.text_style.size, theme.typography.body.size);

    let select = SelectStyle::default_for_theme(&theme);
    assert_eq!(normal_color(&select.background), theme.colors.surface);
    assert_eq!(
        select.menu_background.resolve(),
        theme.colors.surface_overlay
    );
    assert_eq!(
        select.selected_option_background.resolve(),
        theme.colors.primary_container
    );
    assert_eq!(select.radius.resolve(), theme.radius.md);

    let slider = SliderStyle::default_for_theme(&theme);
    assert_eq!(normal_color(&slider.active_track), theme.colors.primary);
    assert_eq!(slider.thumb_size, theme.spacing.md + theme.spacing.xs);
    assert_eq!(slider.text_style.size, theme.typography.label.size);

    let progress = ProgressBarStyle::default_for_theme(&theme);
    assert_eq!(progress.fill_color.resolve(), theme.colors.primary);
    assert_eq!(progress.track_color.resolve(), theme.colors.surface_high);

    let tabs = TabsStyle::default_for_theme(&theme);
    assert_eq!(tabs.indicator_color.resolve(), theme.colors.primary);
    assert_eq!(tabs.tab_bar_background.resolve(), theme.colors.surface_low);
    assert_eq!(tabs.radius.resolve(), theme.radius.lg);

    let list = ListStyle::default_for_theme(&theme);
    assert_eq!(
        list.item_selected_background.resolve(),
        theme.colors.primary_container
    );
    assert_eq!(list.item_radius, theme.radius.md);

    let tree = TreeStyle::default_for_theme(&theme);
    assert_eq!(tree.checkbox_checked_color.resolve(), theme.colors.primary);
    assert_eq!(
        tree.surface.background.as_ref().unwrap().resolve(),
        theme.colors.surface
    );
    assert_eq!(tree.item_radius, theme.radius.md);

    let grid = DataGridStyle::default_for_theme(&theme);
    assert_eq!(grid.cell_focused_border.resolve(), theme.colors.focus_ring);
    assert_eq!(grid.header_background.resolve(), theme.colors.surface_low);
    assert_eq!(
        grid.surface.background.as_ref().unwrap().resolve(),
        theme.colors.surface
    );

    let menu = MenuStyle::default_for_theme(&theme);
    assert_eq!(menu.checked_indicator_color.resolve(), theme.colors.primary);
    assert_eq!(menu.background.resolve(), theme.colors.surface_overlay);
    assert_eq!(menu.radius.resolve(), theme.radius.xl);

    let modal = ModalStyle::default_for_theme(&theme);
    assert_eq!(modal.background.resolve(), theme.colors.surface_overlay);
    assert_eq!(modal.radius.resolve(), theme.radius.xl);
    assert_eq!(modal.title_text_style.weight, theme.typography.label.weight);
}

#[test]
fn style_precedence_keeps_local_mutator_above_theme_components_and_stylesheet() {
    let mut theme = Theme::light();
    theme.components = ComponentThemes::default().button(|style, _| {
        style.radius = Value::Static(dp(2.0));
        style.padding_x = dp(12.0);
    });
    let context = StyleContext::from_theme(&theme);
    let visual = VisualStyle {
        style_id: Some("save".to_string()),
        ..Default::default()
    };

    let sheet = StyleSheet::new()
        .button(ButtonSelector::primary(), |style, _| {
            style.radius = Value::Static(dp(4.0));
        })
        .button_id("save", |style, _| {
            style.padding_x = dp(22.0);
        });
    let local = crate::ui::widget::style::StyleResolver::mutate(
        |context| ButtonStyle::default_for_theme(context.theme, ButtonVariantKind::Primary),
        |style, _| {
            style.radius = Value::Static(dp(6.0));
        },
    );

    let style = resolved_button_style(
        Some(&local),
        &context,
        &sheet,
        &visual,
        ButtonVariantKind::Primary,
    );
    assert_eq!(style.radius.resolve(), dp(6.0));
    assert_eq!(style.padding_x, dp(22.0));
}

#[test]
fn surface_style_fills_empty_visual_fields_without_overriding_explicit_builder_values() {
    let explicit_background = Color::hexa(0x223344FF);
    let explicit_radius = dp(18.0);
    let explicit_opacity = 0.42;
    let theme_background = Color::hexa(0xEECCAAFF);

    let mut background = Some(Value::Static(explicit_background));
    let mut visual = VisualStyle {
        border_radius: Some(Value::Static(explicit_radius)),
        opacity: Value::Static(explicit_opacity),
        ..Default::default()
    };

    let surface = WidgetSurfaceStyle {
        background: Some(Value::Static(theme_background)),
        border_radius: Some(Value::Static(dp(4.0))),
        border_width: Some(Value::Static(dp(2.0))),
        opacity: Value::Static(0.9),
        ..Default::default()
    };

    apply_surface_style(&mut background, &mut visual, &surface);

    assert_eq!(background.unwrap().resolve(), explicit_background);
    assert_eq!(visual.border_radius.unwrap().resolve(), explicit_radius);
    assert_eq!(visual.opacity.resolve(), explicit_opacity);
    assert_eq!(visual.border_width.unwrap().resolve(), dp(2.0));
}

#[test]
fn stylesheet_state_selector_applies_to_matching_runtime_state() {
    let theme = Theme::light();
    let context = StyleContext::from_theme(&theme);
    let visual = VisualStyle::default();
    let hovered = WidgetState {
        hovered: true,
        ..Default::default()
    };
    let hover_color = Color::hexa(0xE05A2AFF);
    let sheet =
        StyleSheet::new().button(ButtonSelector::primary().state(hovered), move |style, _| {
            style.background = StateValue::interactive(
                Value::Static(hover_color),
                Value::Static(hover_color),
                Value::Static(hover_color),
                Value::Static(hover_color),
            );
        });
    let mut style =
        resolved_button_style(None, &context, &sheet, &visual, ButtonVariantKind::Primary);

    let normal = resolve_button_style(&style, WidgetState::default(), &theme);
    assert_ne!(normal.background, hover_color);

    sheet.apply_button_state(
        &mut style,
        &context,
        ButtonVariantKind::Primary,
        &visual,
        hovered,
    );
    let resolved = resolve_button_style(&style, hovered, &theme);
    assert_eq!(resolved.background, hover_color);
}

#[test]
fn local_style_resolves_after_stylesheet_state_patch_without_double_applying_mutator() {
    let theme = Theme::light();
    let context = StyleContext::from_theme(&theme);
    let visual = VisualStyle::default();
    let hovered = WidgetState {
        hovered: true,
        ..Default::default()
    };
    let sheet_color = Color::hexa(0xBB3300FF);
    let local_color = Color::hexa(0x1166DDFF);
    let padding_delta = dp(3.0);

    let sheet =
        StyleSheet::new().button(ButtonSelector::primary().state(hovered), move |style, _| {
            style.background.hovered = Value::Static(sheet_color);
            style.padding_x += dp(20.0);
        });
    let local = crate::ui::widget::style::StyleResolver::mutate(
        |context| ButtonStyle::default_for_theme(context.theme, ButtonVariantKind::Primary),
        move |style, _| {
            style.background.hovered = Value::Static(local_color);
            style.padding_x += padding_delta;
        },
    );

    let base = button_style_base(&context, &sheet, &visual, ButtonVariantKind::Primary);
    let expected_padding = base.padding_x + dp(20.0) + padding_delta;
    let mut state_style = base;
    sheet.apply_button_state(
        &mut state_style,
        &context,
        ButtonVariantKind::Primary,
        &visual,
        hovered,
    );
    let state_style = apply_local_style(Some(&local), state_style, &context, &sheet, &visual);
    let resolved = resolve_button_style(&state_style, hovered, &theme);

    assert_eq!(resolved.background, local_color);
    assert_eq!(state_style.padding_x, expected_padding);
}
