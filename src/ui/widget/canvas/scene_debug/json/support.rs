mod names;

use super::super::*;
pub(super) use self::names::{
    canvas_blend_mode_name, canvas_fill_rule_name, canvas_group_mode_name,
    canvas_item_kind_name, content_fit_name,
};
pub(crate) use self::names::{
    canvas_text_horizontal_align_name, canvas_text_overflow_name,
    canvas_text_vertical_align_name, canvas_text_wrap_name,
};
use self::names::{
    canvas_stroke_alignment_name, canvas_stroke_cap_name, canvas_stroke_join_name,
};

pub(super) fn write_optional_brush_value_json(
    out: &mut String,
    brush: Option<&Value<CanvasBrush>>,
    indent: usize,
) {
    match brush {
        Some(Value::Static(brush)) => write_brush_json(out, brush, indent),
        Some(Value::Signal(_)) => out.push_str("{\"kind\":\"dynamic\"}"),
        None => out.push_str("null"),
    }
}

pub(super) fn write_brush_json(out: &mut String, brush: &CanvasBrush, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    match brush {
        CanvasBrush::Solid(color) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"solid\",\n"));
            out.push_str(&format!(
                "{field_prefix}\"color\": {}",
                json_string(&color_hex(*color))
            ));
        }
        CanvasBrush::LinearGradient(gradient) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"linear_gradient\",\n"));
            out.push_str(&format!("{field_prefix}\"start\": "));
            write_point_json(out, gradient.start, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!("{field_prefix}\"end\": "));
            write_point_json(out, gradient.end, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!("{field_prefix}\"stops\": "));
            write_gradient_stops_json(out, &gradient.stops, indent + 1);
        }
        CanvasBrush::RadialGradient(gradient) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"radial_gradient\",\n"));
            out.push_str(&format!("{field_prefix}\"center\": "));
            write_point_json(out, gradient.center, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!(
                "{field_prefix}\"radius\": {},\n",
                gradient.radius.get()
            ));
            out.push_str(&format!("{field_prefix}\"stops\": "));
            write_gradient_stops_json(out, &gradient.stops, indent + 1);
        }
    }
    out.push_str(&format!("\n{prefix}}}"));
}

fn write_gradient_stops_json(out: &mut String, stops: &[CanvasGradientStop], indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("[\n");
    for (index, stop) in stops.iter().enumerate() {
        out.push_str(&format!("{field_prefix}{{\n"));
        out.push_str(&format!("{field_prefix}  \"offset\": {},\n", stop.offset));
        out.push_str(&format!(
            "{field_prefix}  \"color\": {}\n",
            json_string(&color_hex(stop.color))
        ));
        out.push_str(&format!("{field_prefix}}}"));
        if index + 1 != stops.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{prefix}]"));
}

pub(super) fn write_optional_stroke_json(
    out: &mut String,
    stroke: Option<&CanvasStroke>,
    indent: usize,
) {
    let Some(stroke) = stroke else {
        out.push_str("null");
        return;
    };

    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    out.push_str(&format!(
        "{field_prefix}\"width\": {},\n",
        stroke.width.get()
    ));
    out.push_str(&format!("{field_prefix}\"brush\": "));
    match &stroke.brush {
        Value::Static(brush) => write_brush_json(out, brush, indent + 1),
        Value::Signal(_) => out.push_str("{\"kind\":\"dynamic\"}"),
    }
    out.push_str(",\n");
    out.push_str(&format!(
        "{field_prefix}\"dash_pattern\": {},\n",
        stroke
            .dash_pattern
            .as_ref()
            .map(|pattern| json_dp_array(pattern))
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "{field_prefix}\"dash_offset\": {},\n",
        stroke.dash_offset.get()
    ));
    out.push_str(&format!(
        "{field_prefix}\"line_cap\": {},\n",
        json_string(canvas_stroke_cap_name(stroke.line_cap))
    ));
    out.push_str(&format!(
        "{field_prefix}\"line_join\": {},\n",
        json_string(canvas_stroke_join_name(stroke.line_join))
    ));
    out.push_str(&format!(
        "{field_prefix}\"miter_limit\": {},\n",
        stroke.miter_limit
    ));
    out.push_str(&format!(
        "{field_prefix}\"alignment\": {}\n",
        json_string(canvas_stroke_alignment_name(stroke.alignment))
    ));
    out.push_str(&format!("{prefix}}}"));
}

pub(super) fn write_optional_shadow_value_json(
    out: &mut String,
    shadow: Option<&Value<CanvasShadow>>,
    indent: usize,
) {
    match shadow {
        Some(Value::Static(shadow)) => {
            let prefix = "  ".repeat(indent);
            let field_prefix = "  ".repeat(indent + 1);
            out.push_str("{\n");
            out.push_str(&format!(
                "{field_prefix}\"color\": {},\n",
                json_string(&color_hex(shadow.color))
            ));
            out.push_str(&format!("{field_prefix}\"offset\": "));
            write_point_json(out, shadow.offset, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!("{field_prefix}\"blur\": {}\n", shadow.blur.get()));
            out.push_str(&format!("{prefix}}}"));
        }
        Some(Value::Signal(_)) => out.push_str("{\"kind\":\"dynamic\"}"),
        None => out.push_str("null"),
    }
}

pub(super) fn write_canvas_effects_json(out: &mut String, effects: &[CanvasEffect], indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("[\n");
    for (index, effect) in effects.iter().enumerate() {
        out.push_str(&format!("{field_prefix}{{\n"));
        match effect {
            CanvasEffect::Blur(radius) => {
                out.push_str(&format!("{field_prefix}  \"kind\": \"blur\",\n"));
                out.push_str(&format!("{field_prefix}  \"radius\": {}\n", radius.get()));
            }
            CanvasEffect::ColorFilter(filter) => {
                out.push_str(&format!("{field_prefix}  \"kind\": \"color_filter\",\n"));
                out.push_str(&format!(
                    "{field_prefix}  \"multiply\": {},\n",
                    json_f32_array(&filter.multiply)
                ));
                out.push_str(&format!(
                    "{field_prefix}  \"add\": {}\n",
                    json_f32_array(&filter.add)
                ));
            }
            CanvasEffect::InnerShadow(shadow) => {
                out.push_str(&format!("{field_prefix}  \"kind\": \"inner_shadow\",\n"));
                out.push_str(&format!(
                    "{field_prefix}  \"color\": {},\n",
                    json_string(&color_hex(shadow.color))
                ));
                out.push_str(&format!("{field_prefix}  \"offset\": "));
                write_point_json(out, shadow.offset, indent + 2);
                out.push_str(",\n");
                out.push_str(&format!(
                    "{field_prefix}  \"blur\": {}\n",
                    shadow.blur.get()
                ));
            }
        }
        out.push_str(&format!("{field_prefix}}}"));
        if index + 1 != effects.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{prefix}]"));
}

pub(super) fn write_text_content_json(out: &mut String, content: &CanvasTextContent, indent: usize) {
    match content {
        CanvasTextContent::Plain(text) => {
            out.push_str("{\"kind\":\"plain\",\"text\":");
            out.push_str(&json_string(text));
            out.push('}');
        }
        CanvasTextContent::Rich(spans) => {
            let prefix = "  ".repeat(indent);
            let field_prefix = "  ".repeat(indent + 1);
            out.push_str("{\n");
            out.push_str(&format!("{field_prefix}\"kind\": \"rich\",\n"));
            out.push_str(&format!("{field_prefix}\"spans\": [\n"));
            for (index, span) in spans.iter().enumerate() {
                out.push_str(&format!("{field_prefix}  {{\n"));
                out.push_str(&format!(
                    "{field_prefix}    \"content\": {},\n",
                    json_string(&span.content)
                ));
                out.push_str(&format!("{field_prefix}    \"style\": "));
                write_text_style_json(out, &span.style, indent + 2);
                out.push_str(&format!("\n{field_prefix}  }}"));
                if index + 1 != spans.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&format!("{field_prefix}]\n{prefix}}}"));
        }
    }
}

pub(super) fn write_text_style_json(out: &mut String, style: &CanvasTextStyle, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    out.push_str(&format!(
        "{field_prefix}\"font_family\": {},\n",
        style
            .font_family
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "{field_prefix}\"color\": {},\n",
        json_string(&color_hex(style.color))
    ));
    out.push_str(&format!(
        "{field_prefix}\"font_size\": {},\n",
        style.font_size.get()
    ));
    out.push_str(&format!(
        "{field_prefix}\"font_weight\": {},\n",
        style.font_weight.to_raw()
    ));
    out.push_str(&format!(
        "{field_prefix}\"line_height\": {},\n",
        style
            .line_height
            .map(|value| value.get().to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "{field_prefix}\"letter_spacing\": {}\n",
        style.letter_spacing.get()
    ));
    out.push_str(&format!("{prefix}}}"));
}

pub(super) fn write_paragraph_style_json(
    out: &mut String,
    style: &CanvasParagraphStyle,
    indent: usize,
) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    out.push_str(&format!(
        "{field_prefix}\"wrap\": {},\n",
        json_string(canvas_text_wrap_name(style.wrap))
    ));
    out.push_str(&format!(
        "{field_prefix}\"horizontal_align\": {},\n",
        json_string(canvas_text_horizontal_align_name(style.horizontal_align))
    ));
    out.push_str(&format!(
        "{field_prefix}\"vertical_align\": {},\n",
        json_string(canvas_text_vertical_align_name(style.vertical_align))
    ));
    out.push_str(&format!(
        "{field_prefix}\"overflow\": {}\n",
        json_string(canvas_text_overflow_name(style.overflow))
    ));
    out.push_str(&format!("{prefix}}}"));
}

pub(super) fn write_media_source_json(out: &mut String, source: &MediaSource, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    match source {
        MediaSource::Path(path) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"path\",\n"));
            out.push_str(&format!(
                "{field_prefix}\"value\": {}\n",
                json_string(&path.to_string_lossy())
            ));
        }
        MediaSource::Url(url) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"url\",\n"));
            out.push_str(&format!("{field_prefix}\"value\": {}\n", json_string(url)));
        }
        MediaSource::Bytes(bytes) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"bytes\",\n"));
            out.push_str(&format!("{field_prefix}\"length\": {},\n", bytes.len()));
            out.push_str(&format!(
                "{field_prefix}\"hex\": {}\n",
                json_string(&hex_bytes(bytes.as_slice()))
            ));
        }
    }
    out.push_str(&format!("{prefix}}}"));
}

pub(super) fn write_rect_json(out: &mut String, rect: Rect, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    out.push_str(&format!("{field_prefix}\"x\": {},\n", rect.x.get()));
    out.push_str(&format!("{field_prefix}\"y\": {},\n", rect.y.get()));
    out.push_str(&format!("{field_prefix}\"width\": {},\n", rect.width.get()));
    out.push_str(&format!(
        "{field_prefix}\"height\": {}\n",
        rect.height.get()
    ));
    out.push_str(&format!("{prefix}}}"));
}

pub(super) fn write_point_json(out: &mut String, point_value: Point, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    out.push_str(&format!("{field_prefix}\"x\": {},\n", point_value.x.get()));
    out.push_str(&format!("{field_prefix}\"y\": {}\n", point_value.y.get()));
    out.push_str(&format!("{prefix}}}"));
}

pub(super) fn json_f32_array(values: &[f32]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&value.to_string());
    }
    out.push(']');
    out
}

fn json_dp_array(values: &[Dp]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&value.get().to_string());
    }
    out.push(']');
    out
}

pub(super) fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch <= '\u{1F}' => out.push_str(&format!("\\u{:04X}", ch as u32)),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn color_hex(color: Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.r, color.g, color.b, color.a
    )
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02X}"));
    }
    out
}
