use super::*;

use std::fmt::Write as _;

mod support;

use self::support::*;
pub(crate) use self::support::{
    canvas_text_horizontal_align_name, canvas_text_overflow_name, canvas_text_vertical_align_name,
    canvas_text_wrap_name,
};

pub(crate) fn export_canvas_scene_json(scene: &CanvasScene) -> String {
    let mut out = String::with_capacity(estimate_canvas_scene_json_capacity(scene));
    out.push_str("{\n");
    out.push_str("  \"format\": ");
    write_json_static_string(&mut out, CanvasScene::STABLE_JSON_FORMAT);
    out.push_str(",\n  \"version\": ");
    push_usize(&mut out, CanvasScene::STABLE_JSON_VERSION as usize);
    out.push_str(",\n");
    out.push_str("  \"bounds\": ");
    write_optional_rect_json(&mut out, scene.bounds(), 1);
    out.push_str(",\n  \"items\": [\n");
    for (index, item) in scene.items().iter().enumerate() {
        write_canvas_scene_item_json(&mut out, item, 2);
        if index + 1 != scene.items().len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n}");
    out
}

pub(super) fn write_canvas_scene_debug_json(
    scene: &CanvasScene,
    stats: &CanvasSceneDebugStats,
) -> String {
    let mut out =
        String::with_capacity(128usize.saturating_add(stats.total_items.saturating_mul(512)));
    out.push_str("{\n");
    out.push_str("  \"stats\": ");
    write_debug_stats_json(&mut out, stats, 1);
    out.push_str(",\n  \"nodes\": [\n");
    let mut index_path = Vec::new();
    for (index, item) in scene.items().iter().enumerate() {
        index_path.push(index);
        write_debug_item_json(&mut out, item, 0, &mut index_path, 2);
        index_path.pop();
        if index + 1 != scene.items().len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n}");
    out
}

fn estimate_canvas_scene_json_capacity(scene: &CanvasScene) -> usize {
    128usize.saturating_add(estimate_canvas_items_json_capacity(scene.items()))
}

fn estimate_canvas_items_json_capacity(items: &[CanvasItem]) -> usize {
    items.iter().fold(0usize, |total, item| {
        total.saturating_add(estimate_canvas_item_json_capacity(item))
    })
}

fn estimate_canvas_item_json_capacity(item: &CanvasItem) -> usize {
    let style = item.style();
    let name_len = style
        .name
        .as_ref()
        .map_or(4, |name| name.len().saturating_add(2));
    let style_len = 240usize
        .saturating_add(name_len)
        .saturating_add(style.effects.len().saturating_mul(160));
    let payload_len = match item {
        CanvasItem::Path(path) => 520usize
            .saturating_add(path.path.commands_internal().len().saturating_mul(96))
            .saturating_add(estimate_optional_brush_capacity(path.fill.as_ref()))
            .saturating_add(path.stroke.as_ref().map_or(4, |_| 280))
            .saturating_add(path.shadow.as_ref().map_or(4, |_| 160)),
        CanvasItem::Text(text) => {
            620usize.saturating_add(estimate_text_content_capacity(&text.content))
        }
        CanvasItem::Image(image) => {
            460usize.saturating_add(estimate_media_source_capacity(&image.source))
        }
        CanvasItem::Group(group) => 560usize
            .saturating_add(estimate_group_shape_capacity(&group.shape))
            .saturating_add(estimate_canvas_items_json_capacity(&group.items)),
    };
    style_len.saturating_add(payload_len)
}

fn estimate_text_content_capacity(content: &CanvasTextContent) -> usize {
    match content {
        CanvasTextContent::Plain(text) => text.len().saturating_add(48),
        CanvasTextContent::Rich(spans) => spans.iter().fold(48usize, |total, span| {
            total.saturating_add(span.content.len()).saturating_add(320)
        }),
    }
}

fn estimate_optional_brush_capacity(brush: Option<&Value<CanvasBrush>>) -> usize {
    match brush {
        Some(Value::Static(CanvasBrush::Solid(_))) => 64,
        Some(Value::Static(CanvasBrush::LinearGradient(gradient))) => {
            180usize.saturating_add(gradient.stops.len().saturating_mul(56))
        }
        Some(Value::Static(CanvasBrush::RadialGradient(gradient))) => {
            180usize.saturating_add(gradient.stops.len().saturating_mul(56))
        }
        Some(Value::Signal(_)) => 18,
        None => 4,
    }
}

fn estimate_group_shape_capacity(shape: &CanvasGroupShape) -> usize {
    match shape {
        CanvasGroupShape::Path { path, .. } => {
            160usize.saturating_add(path.commands_internal().len().saturating_mul(96))
        }
    }
}

fn estimate_media_source_capacity(source: &MediaSource) -> usize {
    match source {
        MediaSource::Path(_) => 128,
        MediaSource::Url(url) => url.len().saturating_add(48),
        MediaSource::Bytes(bytes) => bytes.len().saturating_mul(2).saturating_add(56),
    }
}

pub(super) fn write_debug_stats_json(
    out: &mut String,
    stats: &CanvasSceneDebugStats,
    indent: usize,
) {
    out.push_str("{\n");
    push_debug_usize_field(out, indent + 1, "root_items", stats.root_items);
    push_debug_usize_field(out, indent + 1, "total_items", stats.total_items);
    push_debug_usize_field(out, indent + 1, "named_items", stats.named_items);
    push_debug_usize_field(out, indent + 1, "visible_items", stats.visible_items);
    push_debug_usize_field(
        out,
        indent + 1,
        "hit_testable_items",
        stats.hit_testable_items,
    );
    push_debug_usize_field(out, indent + 1, "path_items", stats.path_items);
    push_debug_usize_field(out, indent + 1, "text_items", stats.text_items);
    push_debug_usize_field(out, indent + 1, "image_items", stats.image_items);
    push_debug_usize_field(out, indent + 1, "group_items", stats.group_items);
    push_debug_usize_field(out, indent + 1, "max_depth", stats.max_depth);
    push_indent(out, indent + 1);
    out.push_str("\"bounds\": ");
    write_optional_rect_json(out, stats.bounds, indent + 1);
    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}

fn push_debug_usize_field(out: &mut String, indent: usize, name: &str, value: usize) {
    push_indent(out, indent);
    out.push('"');
    out.push_str(name);
    out.push_str("\": ");
    push_usize(out, value);
    out.push(',');
    out.push('\n');
}

pub(super) fn write_debug_node_json(out: &mut String, node: &CanvasSceneDebugNode, indent: usize) {
    push_indent(out, indent);
    out.push_str("{\n");
    push_indent(out, indent + 1);
    out.push_str("\"id\": ");
    push_u64(out, node.id.get());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"name\": ");
    if let Some(name) = node.name.as_deref() {
        write_json_string(out, name);
    } else {
        out.push_str("null");
    }
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"kind\": \"");
    out.push_str(canvas_item_kind_debug_name(node.kind));
    out.push_str("\",\n");
    push_indent(out, indent + 1);
    out.push_str("\"depth\": ");
    push_usize(out, node.depth);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"index_path\": ");
    write_usize_array_json(out, &node.index_path);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"visible\": ");
    push_bool(out, node.visible);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"hit_test\": ");
    push_bool(out, node.hit_test);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"opacity\": ");
    push_f32(out, node.opacity);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"blend_mode\": \"");
    out.push_str(canvas_blend_mode_debug_name(node.blend_mode));
    out.push_str("\",\n");
    push_indent(out, indent + 1);
    out.push_str("\"bounds\": ");
    write_optional_rect_json(out, node.bounds, indent + 1);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"child_count\": ");
    push_usize(out, node.child_count);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"summary\": ");
    write_json_string(out, &node.summary);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"children\": [");
    if node.children.is_empty() {
        out.push_str("]\n");
    } else {
        out.push('\n');
        for (index, child) in node.children.iter().enumerate() {
            write_debug_node_json(out, child, indent + 2);
            if index + 1 != node.children.len() {
                out.push(',');
            }
            out.push('\n');
        }
        push_indent(out, indent + 1);
        out.push_str("]\n");
    }
    push_indent(out, indent);
    out.push('}');
}

fn write_debug_item_json(
    out: &mut String,
    item: &CanvasItem,
    depth: usize,
    index_path: &mut Vec<usize>,
    indent: usize,
) {
    push_indent(out, indent);
    out.push_str("{\n");
    push_indent(out, indent + 1);
    out.push_str("\"id\": ");
    push_u64(out, item.id().get());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"name\": ");
    if let Some(name) = item.name() {
        write_json_string(out, name);
    } else {
        out.push_str("null");
    }
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"kind\": \"");
    out.push_str(canvas_item_kind_debug_name(item.kind()));
    out.push_str("\",\n");
    push_indent(out, indent + 1);
    out.push_str("\"depth\": ");
    push_usize(out, depth);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"index_path\": ");
    write_usize_array_json(out, index_path);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"visible\": ");
    push_bool(out, item.style().visible);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"hit_test\": ");
    push_bool(out, item.style().hit_test);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"opacity\": ");
    push_f32(out, item.style().opacity);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"blend_mode\": \"");
    out.push_str(canvas_blend_mode_debug_name(item.style().blend_mode));
    out.push_str("\",\n");
    push_indent(out, indent + 1);
    out.push_str("\"bounds\": ");
    write_optional_rect_json(out, item.layout_bounds().map(rect_from_bounds), indent + 1);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"child_count\": ");
    push_usize(out, item.children().len());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"summary\": ");
    write_debug_item_summary_json(out, item);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"children\": [");
    if let CanvasItem::Group(group) = item {
        if !group.items.is_empty() {
            out.push('\n');
            for (index, child) in group.items.iter().enumerate() {
                index_path.push(index);
                write_debug_item_json(out, child, depth + 1, index_path, indent + 2);
                index_path.pop();
                if index + 1 != group.items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            push_indent(out, indent + 1);
        }
    }
    out.push_str("]\n");
    push_indent(out, indent);
    out.push('}');
}

fn write_debug_item_summary_json(out: &mut String, item: &CanvasItem) {
    out.push('"');
    match item {
        CanvasItem::Path(path) => {
            out.push_str("path(fill=");
            push_bool(out, path.fill.is_some());
            out.push_str(", stroke=");
            push_bool(out, path.stroke.is_some());
            out.push_str(", shadow=");
            push_bool(out, path.shadow.is_some());
            out.push(')');
        }
        CanvasItem::Text(text) => {
            out.push_str("text(chars=");
            push_usize(out, text.plain_text_char_count());
            out.push(')');
        }
        CanvasItem::Image(image) => {
            out.push_str("image(fit=");
            out.push_str(content_fit_debug_name(image.fit));
            out.push(')');
        }
        CanvasItem::Group(group) => {
            out.push_str("group(mode=");
            out.push_str(canvas_group_mode_debug_name(&group.mode));
            out.push_str(", items=");
            push_usize(out, group.items.len());
            out.push(')');
        }
    }
    out.push('"');
}

fn write_optional_rect_json(out: &mut String, rect: Option<Rect>, indent: usize) {
    match rect {
        Some(rect) => {
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
        None => out.push_str("null"),
    }
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

fn push_f32(out: &mut String, value: f32) {
    if value.is_finite() && value.fract() == 0.0 {
        if value == 0.0 {
            if value.is_sign_negative() {
                out.push_str("-0");
            } else {
                out.push('0');
            }
            return;
        }

        if value >= i64::MIN as f32 && value <= i64::MAX as f32 {
            push_i64(out, value as i64);
            return;
        }
    }

    let _ = write!(out, "{value}");
}

fn push_i64(out: &mut String, value: i64) {
    if value < 0 {
        out.push('-');
        push_u64(out, value.unsigned_abs());
    } else {
        push_u64(out, value as u64);
    }
}

fn write_usize_array_json(out: &mut String, values: &[usize]) {
    out.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        push_usize(out, *value);
    }
    out.push(']');
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

fn push_u64(out: &mut String, mut value: u64) {
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
    out.push_str(std::str::from_utf8(&buffer[index..]).expect("u64 digits are valid UTF-8"));
}

fn push_bool(out: &mut String, value: bool) {
    out.push_str(if value { "true" } else { "false" });
}

fn canvas_item_kind_debug_name(kind: CanvasItemKind) -> &'static str {
    match kind {
        CanvasItemKind::Path => "Path",
        CanvasItemKind::Text => "Text",
        CanvasItemKind::Image => "Image",
        CanvasItemKind::Group => "Group",
    }
}

fn canvas_blend_mode_debug_name(mode: CanvasBlendMode) -> &'static str {
    match mode {
        CanvasBlendMode::Normal => "Normal",
        CanvasBlendMode::Multiply => "Multiply",
        CanvasBlendMode::Screen => "Screen",
        CanvasBlendMode::Overlay => "Overlay",
        CanvasBlendMode::Darken => "Darken",
        CanvasBlendMode::Lighten => "Lighten",
        CanvasBlendMode::ColorDodge => "ColorDodge",
        CanvasBlendMode::ColorBurn => "ColorBurn",
        CanvasBlendMode::HardLight => "HardLight",
        CanvasBlendMode::SoftLight => "SoftLight",
        CanvasBlendMode::Difference => "Difference",
        CanvasBlendMode::Exclusion => "Exclusion",
        CanvasBlendMode::Plus => "Plus",
    }
}

fn content_fit_debug_name(fit: ContentFit) -> &'static str {
    match fit {
        ContentFit::Contain => "Contain",
        ContentFit::Cover => "Cover",
        ContentFit::Fill => "Fill",
    }
}

fn canvas_group_mode_debug_name(mode: &CanvasGroupMode) -> &'static str {
    match mode {
        CanvasGroupMode::Clip => "Clip",
        CanvasGroupMode::Mask => "Mask",
    }
}

fn write_json_string(out: &mut String, value: &str) {
    out.push('"');
    write_json_string_contents(out, value);
    out.push('"');
}

fn write_json_static_string(out: &mut String, value: &str) {
    out.push('"');
    out.push_str(value);
    out.push('"');
}

fn write_json_string_contents(out: &mut String, value: &str) {
    if !requires_json_escape(value) {
        out.push_str(value);
        return;
    }

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
}

fn requires_json_escape(value: &str) -> bool {
    value
        .as_bytes()
        .iter()
        .any(|&byte| matches!(byte, b'\\' | b'"' | 0x00..=0x1F))
}

fn write_canvas_scene_item_json(out: &mut String, item: &CanvasItem, indent: usize) {
    push_indent(out, indent);
    out.push_str("{\n");
    push_indent(out, indent + 1);
    out.push_str("\"id\": ");
    push_u64(out, item.id().get());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"name\": ");
    if let Some(name) = item.name() {
        write_json_string(out, name);
    } else {
        out.push_str("null");
    }
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"kind\": ");
    write_json_static_string(out, canvas_item_kind_name(item.kind()));
    out.push_str(",\n");
    write_canvas_item_style_json(out, item.style(), indent + 1);
    out.push_str(",\n");

    match item {
        CanvasItem::Path(path) => write_canvas_path_payload_json(out, path, indent + 1),
        CanvasItem::Text(text) => write_canvas_text_payload_json(out, text, indent + 1),
        CanvasItem::Image(image) => write_canvas_image_payload_json(out, image, indent + 1),
        CanvasItem::Group(group) => write_canvas_group_payload_json(out, group, indent + 1),
    }

    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}

fn write_canvas_item_style_json(out: &mut String, style: &CanvasItemStyle, indent: usize) {
    push_indent(out, indent);
    out.push_str("\"style\": {\n");
    push_indent(out, indent + 1);
    out.push_str("\"transform\": ");
    write_f32_array_json(out, &style.transform.matrix);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"opacity\": ");
    push_f32(out, style.opacity);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"blend_mode\": ");
    write_json_static_string(out, canvas_blend_mode_name(style.blend_mode));
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"isolation\": ");
    push_bool(out, style.isolation);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"visible\": ");
    push_bool(out, style.visible);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"hit_test\": ");
    push_bool(out, style.hit_test);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"effects\": ");
    write_canvas_effects_json(out, &style.effects, indent + 1);
    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}

fn write_canvas_path_payload_json(out: &mut String, path: &CanvasPath, indent: usize) {
    push_indent(out, indent);
    out.push_str("\"payload\": {\n");
    push_indent(out, indent + 1);
    out.push_str("\"fill_rule\": ");
    write_json_static_string(out, canvas_fill_rule_name(path.fill_rule));
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"path\": ");
    write_path_builder_json(out, &path.path, indent + 1);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"fill\": ");
    write_optional_brush_value_json(out, path.fill.as_ref(), indent + 1);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"stroke\": ");
    write_optional_stroke_json(out, path.stroke.as_ref(), indent + 1);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"shadow\": ");
    write_optional_shadow_value_json(out, path.shadow.as_ref(), indent + 1);
    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}

fn write_canvas_text_payload_json(out: &mut String, text: &CanvasText, indent: usize) {
    push_indent(out, indent);
    out.push_str("\"payload\": {\n");
    push_indent(out, indent + 1);
    out.push_str("\"frame\": ");
    write_rect_json(out, text.frame, indent + 1);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"content\": ");
    write_text_content_json(out, &text.content, indent + 1);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"text_style\": ");
    write_text_style_json(out, &text.text_style, indent + 1);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"paragraph_style\": ");
    write_paragraph_style_json(out, &text.paragraph_style, indent + 1);
    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}

fn write_canvas_image_payload_json(out: &mut String, image: &CanvasImage, indent: usize) {
    push_indent(out, indent);
    out.push_str("\"payload\": {\n");
    push_indent(out, indent + 1);
    out.push_str("\"frame\": ");
    write_rect_json(out, image.frame, indent + 1);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"source\": ");
    write_media_source_json(out, &image.source, indent + 1);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"fit\": ");
    write_json_static_string(out, content_fit_name(image.fit));
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"corner_radius\": ");
    push_f32(out, image.corner_radius.get());
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"source_rect\": ");
    write_optional_rect_json(out, image.source_rect, indent + 1);
    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}

fn write_canvas_group_payload_json(out: &mut String, group: &CanvasGroup, indent: usize) {
    push_indent(out, indent);
    out.push_str("\"payload\": {\n");
    push_indent(out, indent + 1);
    out.push_str("\"mode\": ");
    write_json_static_string(out, canvas_group_mode_name(&group.mode));
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"shape\": ");
    write_group_shape_json(out, &group.shape, indent + 1);
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"items\": [\n");
    for (index, child) in group.items.iter().enumerate() {
        write_canvas_scene_item_json(out, child, indent + 2);
        if index + 1 != group.items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(out, indent + 1);
    out.push_str("]\n");
    push_indent(out, indent);
    out.push('}');
}

fn write_group_shape_json(out: &mut String, shape: &CanvasGroupShape, indent: usize) {
    match shape {
        CanvasGroupShape::Path { path, fill_rule } => {
            out.push_str("{\n");
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"path\",\n");
            push_indent(out, indent + 1);
            out.push_str("\"fill_rule\": ");
            write_json_static_string(out, canvas_fill_rule_name(*fill_rule));
            out.push_str(",\n");
            push_indent(out, indent + 1);
            out.push_str("\"path\": ");
            write_path_builder_json(out, path, indent + 1);
            out.push('\n');
            push_indent(out, indent);
            out.push('}');
        }
    }
}

fn write_path_builder_json(out: &mut String, path: &PathBuilder, indent: usize) {
    out.push_str("{\n");
    push_indent(out, indent + 1);
    out.push_str("\"fill_rule\": ");
    write_json_static_string(out, canvas_fill_rule_name(path.fill_rule));
    out.push_str(",\n");
    push_indent(out, indent + 1);
    out.push_str("\"commands\": [\n");
    for (index, command) in path.commands.iter().enumerate() {
        write_path_command_json(out, command, indent + 2);
        if index + 1 != path.commands.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(out, indent + 1);
    out.push_str("]\n");
    push_indent(out, indent);
    out.push('}');
}

fn write_path_command_json(out: &mut String, command: &PathCommand, indent: usize) {
    push_indent(out, indent);
    out.push_str("{\n");
    match command {
        PathCommand::MoveTo(point_value) => {
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"move_to\",\n");
            push_indent(out, indent + 1);
            out.push_str("\"point\": ");
            write_point_json(out, *point_value, indent + 1);
        }
        PathCommand::LineTo(point_value) => {
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"line_to\",\n");
            push_indent(out, indent + 1);
            out.push_str("\"point\": ");
            write_point_json(out, *point_value, indent + 1);
        }
        PathCommand::QuadTo { ctrl, to } => {
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"quad_to\",\n");
            push_indent(out, indent + 1);
            out.push_str("\"ctrl\": ");
            write_point_json(out, *ctrl, indent + 1);
            out.push_str(",\n");
            push_indent(out, indent + 1);
            out.push_str("\"to\": ");
            write_point_json(out, *to, indent + 1);
        }
        PathCommand::CubicTo { ctrl1, ctrl2, to } => {
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"cubic_to\",\n");
            push_indent(out, indent + 1);
            out.push_str("\"ctrl1\": ");
            write_point_json(out, *ctrl1, indent + 1);
            out.push_str(",\n");
            push_indent(out, indent + 1);
            out.push_str("\"ctrl2\": ");
            write_point_json(out, *ctrl2, indent + 1);
            out.push_str(",\n");
            push_indent(out, indent + 1);
            out.push_str("\"to\": ");
            write_point_json(out, *to, indent + 1);
        }
        PathCommand::Close => {
            push_indent(out, indent + 1);
            out.push_str("\"kind\": \"close\"\n");
            push_indent(out, indent);
            out.push('}');
            return;
        }
    }
    out.push('\n');
    push_indent(out, indent);
    out.push('}');
}
