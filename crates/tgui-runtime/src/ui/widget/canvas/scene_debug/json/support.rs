mod names;

pub(super) use self::names::{
    canvas_blend_mode_name, canvas_fill_rule_name, canvas_group_mode_name, canvas_item_kind_name,
    content_fit_name,
};
use self::names::{canvas_stroke_alignment_name, canvas_stroke_cap_name, canvas_stroke_join_name};
pub(crate) use self::names::{
    canvas_text_horizontal_align_name, canvas_text_overflow_name, canvas_text_vertical_align_name,
    canvas_text_wrap_name,
};
use super::super::*;
use super::{push_f32, push_indent, write_json_static_string, write_json_string};

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
    out.push_str("{\n");
    match brush {
        CanvasBrush::Solid(color) => {
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"solid\",\n");
            push_indent(out, indent + 1);
            out.push_str("\"color\": ");
            write_color_hex_json(out, *color);
        }
        CanvasBrush::LinearGradient(gradient) => {
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"linear_gradient\",\n");
            push_indent(out, indent + 1);
            out.push_str("\"start\": ");
            write_point_json(out, gradient.start, indent + 1);
            out.push_str(",\n");
            push_indent(out, indent + 1);
            out.push_str("\"end\": ");
            write_point_json(out, gradient.end, indent + 1);
            out.push_str(",\n");
            push_indent(out, indent + 1);
            out.push_str("\"stops\": ");
            write_gradient_stops_json(out, &gradient.stops, indent + 1);
        }
        CanvasBrush::RadialGradient(gradient) => {
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"radial_gradient\",\n");
            push_indent(out, indent + 1);
            out.push_str("\"center\": ");
            write_point_json(out, gradient.center, indent + 1);
            out.push_str(",\n");
            push_indent(out, indent + 1);
            out.push_str("\"radius\": ");
            push_f32(out, gradient.radius.get());
            out.push_str(",\n");
            push_indent(out, indent + 1);
            out.push_str("\"stops\": ");
            write_gradient_stops_json(out, &gradient.stops, indent + 1);
        }
    }
    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}

fn write_gradient_stops_json(out: &mut String, stops: &[CanvasGradientStop], indent: usize) {
    out.push_str("[\n");
    for (index, stop) in stops.iter().enumerate() {
        push_indent(out, indent + 1);
        out.push_str("{\n");
        push_indent(out, indent + 2);
        out.push_str("\"offset\": ");
        push_f32(out, stop.offset);
        out.push_str(",\n");
        push_indent(out, indent + 2);
        out.push_str("\"color\": ");
        write_color_hex_json(out, stop.color);
        out.push('\n');
        push_indent(out, indent + 1);
        out.push('}');
        if index + 1 != stops.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(out, indent);
    out.push(']');
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

    out.push_str("{\n");
    push_indent(out, indent + 1);
    out.push_str("\"width\": ");
    push_f32(out, stroke.width.get());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"brush\": ");
    match &stroke.brush {
        Value::Static(brush) => write_brush_json(out, brush, indent + 1),
        Value::Signal(_) => out.push_str("{\"kind\":\"dynamic\"}"),
    }
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"dash_pattern\": ");
    if let Some(pattern) = &stroke.dash_pattern {
        write_dp_array_json(out, pattern);
    } else {
        out.push_str("null");
    }
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"dash_offset\": ");
    push_f32(out, stroke.dash_offset.get());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"line_cap\": ");
    write_json_static_string(out, canvas_stroke_cap_name(stroke.line_cap));
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"line_join\": ");
    write_json_static_string(out, canvas_stroke_join_name(stroke.line_join));
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"miter_limit\": ");
    push_f32(out, stroke.miter_limit);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"alignment\": ");
    write_json_static_string(out, canvas_stroke_alignment_name(stroke.alignment));
    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}

pub(super) fn write_optional_shadow_value_json(
    out: &mut String,
    shadow: Option<&Value<CanvasShadow>>,
    indent: usize,
) {
    match shadow {
        Some(Value::Static(shadow)) => {
            out.push_str("{\n");
            push_indent(out, indent + 1);
            out.push_str("\"color\": ");
            write_color_hex_json(out, shadow.color);
            out.push_str(",\n");
            push_indent(out, indent + 1);
            out.push_str("\"offset\": ");
            write_point_json(out, shadow.offset, indent + 1);
            out.push_str(",\n");
            push_indent(out, indent + 1);
            out.push_str("\"blur\": ");
            push_f32(out, shadow.blur.get());
            out.push('\n');
            push_indent(out, indent);
            out.push('}');
        }
        Some(Value::Signal(_)) => out.push_str("{\"kind\":\"dynamic\"}"),
        None => out.push_str("null"),
    }
}

pub(super) fn write_canvas_effects_json(out: &mut String, effects: &[CanvasEffect], indent: usize) {
    out.push_str("[\n");
    for (index, effect) in effects.iter().enumerate() {
        push_indent(out, indent + 1);
        out.push_str("{\n");
        match effect {
            CanvasEffect::Blur(radius) => {
                push_indent(out, indent + 2);
                out.push_str("\"kind\": \"blur\",\n");
                push_indent(out, indent + 2);
                out.push_str("\"radius\": ");
                push_f32(out, radius.get());
                out.push('\n');
            }
            CanvasEffect::ColorFilter(filter) => {
                push_indent(out, indent + 2);
                out.push_str("\"kind\": \"color_filter\",\n");
                push_indent(out, indent + 2);
                out.push_str("\"multiply\": ");
                write_f32_array_json(out, &filter.multiply);
                out.push_str(",\n");
                push_indent(out, indent + 2);
                out.push_str("\"add\": ");
                write_f32_array_json(out, &filter.add);
                out.push('\n');
            }
            CanvasEffect::InnerShadow(shadow) => {
                push_indent(out, indent + 2);
                out.push_str("\"kind\": \"inner_shadow\",\n");
                push_indent(out, indent + 2);
                out.push_str("\"color\": ");
                write_color_hex_json(out, shadow.color);
                out.push_str(",\n");
                push_indent(out, indent + 2);
                out.push_str("\"offset\": ");
                write_point_json(out, shadow.offset, indent + 2);
                out.push_str(",\n");
                push_indent(out, indent + 2);
                out.push_str("\"blur\": ");
                push_f32(out, shadow.blur.get());
                out.push('\n');
            }
        }
        push_indent(out, indent + 1);
        out.push('}');
        if index + 1 != effects.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(out, indent);
    out.push(']');
}

pub(super) fn write_text_content_json(
    out: &mut String,
    content: &CanvasTextContent,
    indent: usize,
) {
    match content {
        CanvasTextContent::Plain(text) => {
            out.push_str("{\"kind\":\"plain\",\"text\":");
            write_json_string(out, text);
            out.push('}');
        }
        CanvasTextContent::Rich(spans) => {
            out.push_str("{\n");
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"rich\",\n");
            push_indent(out, indent + 1);
            out.push_str("\"spans\": [\n");
            for (index, span) in spans.iter().enumerate() {
                push_indent(out, indent + 2);
                out.push_str("{\n");
                push_indent(out, indent + 3);
                out.push_str("\"content\": ");
                write_json_string(out, &span.content);
                out.push_str(",\n");
                push_indent(out, indent + 3);
                out.push_str("\"style\": ");
                write_text_style_json(out, &span.style, indent + 2);
                out.push('\n');
                push_indent(out, indent + 2);
                out.push('}');
                if index + 1 != spans.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            push_indent(out, indent + 1);
            out.push_str("]\n");
            push_indent(out, indent);
            out.push('}');
        }
    }
}

pub(super) fn write_text_style_json(out: &mut String, style: &CanvasTextStyle, indent: usize) {
    out.push_str("{\n");
    push_indent(out, indent + 1);
    out.push_str("\"font_family\": ");
    if let Some(font_family) = style.font_family.as_deref() {
        write_json_string(out, font_family);
    } else {
        out.push_str("null");
    }
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"color\": ");
    write_color_hex_json(out, style.color);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"font_size\": ");
    push_f32(out, style.font_size.get());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"font_weight\": ");
    push_u16(out, style.font_weight.to_raw());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"line_height\": ");
    if let Some(line_height) = style.line_height {
        push_f32(out, line_height.get());
    } else {
        out.push_str("null");
    }
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"letter_spacing\": ");
    push_f32(out, style.letter_spacing.get());
    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}

pub(super) fn write_paragraph_style_json(
    out: &mut String,
    style: &CanvasParagraphStyle,
    indent: usize,
) {
    out.push_str("{\n");
    push_indent(out, indent + 1);
    out.push_str("\"wrap\": ");
    write_json_static_string(out, canvas_text_wrap_name(style.wrap));
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"horizontal_align\": ");
    write_json_static_string(
        out,
        canvas_text_horizontal_align_name(style.horizontal_align),
    );
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"vertical_align\": ");
    write_json_static_string(out, canvas_text_vertical_align_name(style.vertical_align));
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"overflow\": ");
    write_json_static_string(out, canvas_text_overflow_name(style.overflow));
    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}

pub(super) fn write_media_source_json(out: &mut String, source: &MediaSource, indent: usize) {
    out.push_str("{\n");
    match source {
        MediaSource::Path(path) => {
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"path\",\n");
            push_indent(out, indent + 1);
            out.push_str("\"value\": ");
            write_json_string(out, &path.to_string_lossy());
            out.push('\n');
        }
        MediaSource::Url(url) => {
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"url\",\n");
            push_indent(out, indent + 1);
            out.push_str("\"value\": ");
            write_json_string(out, url);
            out.push('\n');
        }
        MediaSource::Bytes(bytes) => {
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"bytes\",\n");
            push_indent(out, indent + 1);
            out.push_str("\"length\": ");
            push_usize(out, bytes.len());
            out.push_str(",\n");
            push_indent(out, indent + 1);
            out.push_str("\"hex\": ");
            write_hex_bytes_json_string(out, bytes.as_slice());
            out.push('\n');
        }
    }
    push_indent(out, indent);
    out.push('}');
}

pub(super) fn write_rect_json(out: &mut String, rect: Rect, indent: usize) {
    out.push_str("{\n");
    push_indent(out, indent + 1);
    out.push_str("\"x\": ");
    push_f32(out, rect.x.get());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"y\": ");
    push_f32(out, rect.y.get());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"width\": ");
    push_f32(out, rect.width.get());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"height\": ");
    push_f32(out, rect.height.get());
    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}

pub(super) fn write_point_json(out: &mut String, point_value: Point, indent: usize) {
    out.push_str("{\n");
    push_indent(out, indent + 1);
    out.push_str("\"x\": ");
    push_f32(out, point_value.x.get());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"y\": ");
    push_f32(out, point_value.y.get());
    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}

pub(super) fn write_f32_array_json(out: &mut String, values: &[f32]) {
    out.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        push_f32(out, *value);
    }
    out.push(']');
}

fn write_dp_array_json(out: &mut String, values: &[Dp]) {
    out.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        push_f32(out, value.get());
    }
    out.push(']');
}

fn push_u16(out: &mut String, mut value: u16) {
    if value == 0 {
        out.push('0');
        return;
    }

    let mut buffer = [0_u8; 5];
    let mut index = buffer.len();
    while value > 0 {
        index -= 1;
        buffer[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    out.push_str(std::str::from_utf8(&buffer[index..]).expect("u16 digits are valid UTF-8"));
}

fn push_usize(out: &mut String, mut value: usize) {
    if value == 0 {
        out.push('0');
        return;
    }

    let mut buffer = [0_u8; 20];
    let mut index = buffer.len();
    while value > 0 {
        index -= 1;
        buffer[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    out.push_str(std::str::from_utf8(&buffer[index..]).expect("usize digits are valid UTF-8"));
}

fn write_hex_bytes_json_string(out: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    out.push('"');
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0F) as usize] as char);
    }
    out.push('"');
}

fn write_color_hex_json(out: &mut String, color: Color) {
    out.push('"');
    out.push('#');
    push_hex_byte(out, color.r);
    push_hex_byte(out, color.g);
    push_hex_byte(out, color.b);
    push_hex_byte(out, color.a);
    out.push('"');
}

fn push_hex_byte(out: &mut String, byte: u8) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    out.push(HEX[(byte >> 4) as usize] as char);
    out.push(HEX[(byte & 0x0F) as usize] as char);
}
