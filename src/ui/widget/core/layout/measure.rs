use super::super::*;

pub(crate) fn measure_text_content(
    text: &Text,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32) {
    let default_style = &theme.typography.body;
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    font_manager.measure_text(
        &text.content.resolve(),
        TextFontRequest {
            preferred_font: text
                .font_family
                .as_deref()
                .or(default_style.font_family.as_deref()),
            weight: text.font_weight.unwrap_or(default_style.weight),
        },
        font_size,
        line_height,
        letter_spacing,
    )
}

pub(crate) fn text_from_content(content: impl IntoTextContent) -> Text {
    Text::new(content)
}

pub(crate) fn measure_checkbox_content(
    label: Option<&Value<String>>,
    checkbox_style: &ResolvedCheckboxStyle,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32) {
    let size = units.resolve_dp(checkbox_style.size);
    let Some(label) = label else {
        return (size, size);
    };

    let label = checkbox_label_with_theme(label, checkbox_style);
    let label_size = measure_text_content(&label, font_manager, theme, units);
    (
        size + units.resolve_dp(checkbox_style.label_gap) + label_size.0,
        size.max(label_size.1),
    )
}

pub(crate) fn checkbox_label_with_theme(
    label: &Value<String>,
    checkbox_style: &ResolvedCheckboxStyle,
) -> Text {
    let mut label = text_from_content(label.clone());
    if label.font_family.is_none() {
        label.font_family = checkbox_style.text_style.font_family.clone();
    }
    if label.font_size.is_none() {
        label.font_size = Some(checkbox_style.text_style.size);
    }
    if label.line_height.is_none() {
        label.line_height = checkbox_style.text_style.line_height;
    }
    if label.font_weight.is_none() {
        label.font_weight = Some(checkbox_style.text_style.weight);
    }
    if label.letter_spacing.is_none() {
        label.letter_spacing = checkbox_style.text_style.letter_spacing;
    }
    label
}

pub(crate) fn measure_radio_content(
    label: Option<&Value<String>>,
    radio_style: &ResolvedRadioStyle,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32) {
    let size = units.resolve_dp(radio_style.size);
    let Some(label) = label else {
        return (size, size);
    };

    let label = radio_label_with_theme(label, radio_style);
    let label_size = measure_text_content(&label, font_manager, theme, units);
    (
        size + units.resolve_dp(radio_style.label_gap) + label_size.0,
        size.max(label_size.1),
    )
}

pub(crate) fn radio_label_with_theme(
    label: &Value<String>,
    radio_style: &ResolvedRadioStyle,
) -> Text {
    let mut label = text_from_content(label.clone());
    if label.font_family.is_none() {
        label.font_family = radio_style.text_style.font_family.clone();
    }
    if label.font_size.is_none() {
        label.font_size = Some(radio_style.text_style.size);
    }
    if label.line_height.is_none() {
        label.line_height = radio_style.text_style.line_height;
    }
    if label.font_weight.is_none() {
        label.font_weight = Some(radio_style.text_style.weight);
    }
    if label.letter_spacing.is_none() {
        label.letter_spacing = radio_style.text_style.letter_spacing;
    }
    label
}

pub(crate) fn default_layout_padding<VM>(element: &ResolvedElement<VM>, _theme: &Theme) -> Insets {
    match &element.kind {
        ResolvedWidgetKind::Button { style, .. } => {
            Insets::symmetric(style.padding_x, style.padding_y)
        }
        ResolvedWidgetKind::Select { style, .. } => {
            Insets::symmetric(style.padding_x, style.padding_y)
        }
        ResolvedWidgetKind::TextEditor { style, .. } => {
            Insets::symmetric(style.padding_x, style.padding_y)
        }
        ResolvedWidgetKind::Switch { style, .. } => style.padding,
        ResolvedWidgetKind::Slider { .. } => Insets::ZERO,
        ResolvedWidgetKind::Checkbox { .. } => Insets::ZERO,
        ResolvedWidgetKind::Radio { .. } => Insets::ZERO,
        ResolvedWidgetKind::ProgressBar { .. } => Insets::ZERO,
        ResolvedWidgetKind::Spinner { .. } => Insets::ZERO,
        ResolvedWidgetKind::Text { .. } => Insets::ZERO,
        ResolvedWidgetKind::Container { .. } => Insets::ZERO,
        ResolvedWidgetKind::Virtual { .. } => Insets::ZERO,
        #[cfg(feature = "audio")]
        ResolvedWidgetKind::Audio { .. } => Insets::ZERO,
        ResolvedWidgetKind::Image { .. } => Insets::ZERO,
        ResolvedWidgetKind::Canvas { .. } => Insets::ZERO,
        ResolvedWidgetKind::ToastHost { .. } => Insets::ZERO,
        #[cfg(feature = "video")]
        ResolvedWidgetKind::VideoSurface { .. } => Insets::ZERO,
    }
}

pub(crate) fn resolved_text_metrics(
    text: &Text,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32, f32) {
    let default_style = &theme.typography.body;
    let default_size = default_style.size.max(sp(1.0));
    let default_line_height_sp = text
        .line_height
        .or(default_style.line_height)
        .unwrap_or(text.font_size.unwrap_or(default_style.size) * 1.25);
    let font_size = units.resolve_sp(text.font_size.unwrap_or(default_size));
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
        text.letter_spacing
            .unwrap_or(default_style.letter_spacing.unwrap_or(Sp::ZERO)),
    );
    (font_size, line_height, letter_spacing)
}

pub(crate) fn text_with_typography(
    content: impl IntoTextContent,
    style: &crate::ui::theme::TextStyle,
) -> Text {
    let mut text = text_from_content(content);
    text.font_family = style.font_family.clone();
    text.font_size = Some(style.size);
    text.line_height = style.line_height;
    text.font_weight = Some(style.weight);
    text.letter_spacing = style.letter_spacing;
    text
}

pub(crate) fn measure_media_content(
    known_dimensions: TaffySize<Option<f32>>,
    aspect_ratio: Option<f32>,
    intrinsic_size: IntrinsicSize,
) -> (f32, f32) {
    let ratio = aspect_ratio
        .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
        .or_else(|| intrinsic_size.aspect_ratio());

    match (known_dimensions.width, known_dimensions.height, ratio) {
        (Some(width), Some(height), _) => (width, height),
        (Some(width), None, Some(ratio)) => (width, width / ratio),
        (None, Some(height), Some(ratio)) => (height * ratio, height),
        (Some(width), None, None) => (width, intrinsic_size.height),
        (None, Some(height), None) => (intrinsic_size.width, height),
        (None, None, _) => (intrinsic_size.width, intrinsic_size.height),
    }
}

pub(crate) fn progress_bar_label_with_theme(
    label: &Value<String>,
    progress_style: &crate::ui::widget::style::ProgressBarStyle,
) -> Text {
    text_with_typography(label.clone(), &progress_style.text_style)
}
