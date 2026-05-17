use super::*;

mod support;

pub(crate) use self::support::{
    canvas_text_horizontal_align_name, canvas_text_overflow_name,
    canvas_text_vertical_align_name, canvas_text_wrap_name,
};
use self::support::*;

pub(crate) fn export_canvas_scene_json(scene: &CanvasScene) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"format\": {},\n",
        json_string(CanvasScene::STABLE_JSON_FORMAT)
    ));
    out.push_str(&format!(
        "  \"version\": {},\n",
        CanvasScene::STABLE_JSON_VERSION
    ));
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

pub(super) fn write_debug_stats_json(
    out: &mut String,
    stats: &CanvasSceneDebugStats,
    indent: usize,
) {
    out.push_str("{\n");
    let prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}\"root_items\": {},\n", stats.root_items));
    out.push_str(&format!(
        "{prefix}\"total_items\": {},\n",
        stats.total_items
    ));
    out.push_str(&format!(
        "{prefix}\"named_items\": {},\n",
        stats.named_items
    ));
    out.push_str(&format!(
        "{prefix}\"visible_items\": {},\n",
        stats.visible_items
    ));
    out.push_str(&format!(
        "{prefix}\"hit_testable_items\": {},\n",
        stats.hit_testable_items
    ));
    out.push_str(&format!("{prefix}\"path_items\": {},\n", stats.path_items));
    out.push_str(&format!("{prefix}\"text_items\": {},\n", stats.text_items));
    out.push_str(&format!(
        "{prefix}\"image_items\": {},\n",
        stats.image_items
    ));
    out.push_str(&format!(
        "{prefix}\"group_items\": {},\n",
        stats.group_items
    ));
    out.push_str(&format!("{prefix}\"max_depth\": {},\n", stats.max_depth));
    out.push_str(&format!("{prefix}\"bounds\": "));
    write_optional_rect_json(out, stats.bounds, indent + 1);
    out.push_str(&format!("\n{}", "  ".repeat(indent)));
    out.push('}');
}

pub(super) fn write_debug_node_json(out: &mut String, node: &CanvasSceneDebugNode, indent: usize) {
    let prefix = "  ".repeat(indent);
    out.push_str(&format!("{prefix}{{\n"));
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{field_prefix}\"id\": {},\n", node.id.get()));
    out.push_str(&format!(
        "{field_prefix}\"name\": {},\n",
        node.name
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!("{field_prefix}\"kind\": \"{:?}\",\n", node.kind));
    out.push_str(&format!("{field_prefix}\"depth\": {},\n", node.depth));
    out.push_str(&format!(
        "{field_prefix}\"index_path\": {},\n",
        json_usize_array(&node.index_path)
    ));
    out.push_str(&format!("{field_prefix}\"visible\": {},\n", node.visible));
    out.push_str(&format!("{field_prefix}\"hit_test\": {},\n", node.hit_test));
    out.push_str(&format!("{field_prefix}\"opacity\": {},\n", node.opacity));
    out.push_str(&format!(
        "{field_prefix}\"blend_mode\": \"{:?}\",\n",
        node.blend_mode
    ));
    out.push_str(&format!("{field_prefix}\"bounds\": "));
    write_optional_rect_json(out, node.bounds, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!(
        "{field_prefix}\"child_count\": {},\n",
        node.child_count
    ));
    out.push_str(&format!(
        "{field_prefix}\"summary\": {},\n",
        json_string(&node.summary)
    ));
    out.push_str(&format!("{field_prefix}\"children\": ["));
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
        out.push_str(&format!("{field_prefix}]\n"));
    }
    out.push_str(&format!("{prefix}}}"));
}

fn write_optional_rect_json(out: &mut String, rect: Option<Rect>, indent: usize) {
    match rect {
        Some(rect) => {
            let prefix = "  ".repeat(indent + 1);
            out.push_str("{\n");
            out.push_str(&format!("{prefix}\"x\": {},\n", rect.x.get()));
            out.push_str(&format!("{prefix}\"y\": {},\n", rect.y.get()));
            out.push_str(&format!("{prefix}\"width\": {},\n", rect.width.get()));
            out.push_str(&format!("{prefix}\"height\": {}\n", rect.height.get()));
            out.push_str(&format!("{}{}", "  ".repeat(indent), "}"));
        }
        None => out.push_str("null"),
    }
}

fn json_usize_array(values: &[usize]) -> String {
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

fn json_string(value: &str) -> String {
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

fn write_canvas_scene_item_json(out: &mut String, item: &CanvasItem, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}{{\n"));
    out.push_str(&format!("{field_prefix}\"id\": {},\n", item.id().get()));
    out.push_str(&format!(
        "{field_prefix}\"name\": {},\n",
        item.name()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "{field_prefix}\"kind\": {},\n",
        json_string(canvas_item_kind_name(item.kind()))
    ));
    write_canvas_item_style_json(out, item.style(), indent + 1);
    out.push_str(",\n");

    match item {
        CanvasItem::Path(path) => write_canvas_path_payload_json(out, path, indent + 1),
        CanvasItem::Text(text) => write_canvas_text_payload_json(out, text, indent + 1),
        CanvasItem::Image(image) => write_canvas_image_payload_json(out, image, indent + 1),
        CanvasItem::Group(group) => write_canvas_group_payload_json(out, group, indent + 1),
    }

    out.push('\n');
    out.push_str(&format!("{prefix}}}"));
}

fn write_canvas_item_style_json(out: &mut String, style: &CanvasItemStyle, indent: usize) {
    let prefix = "  ".repeat(indent);
    out.push_str(&format!("{prefix}\"style\": {{\n"));
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!(
        "{field_prefix}\"transform\": {},\n",
        json_f32_array(&style.transform.matrix)
    ));
    out.push_str(&format!("{field_prefix}\"opacity\": {},\n", style.opacity));
    out.push_str(&format!(
        "{field_prefix}\"blend_mode\": {},\n",
        json_string(canvas_blend_mode_name(style.blend_mode))
    ));
    out.push_str(&format!(
        "{field_prefix}\"isolation\": {},\n",
        style.isolation
    ));
    out.push_str(&format!("{field_prefix}\"visible\": {},\n", style.visible));
    out.push_str(&format!(
        "{field_prefix}\"hit_test\": {},\n",
        style.hit_test
    ));
    out.push_str(&format!("{field_prefix}\"effects\": "));
    write_canvas_effects_json(out, &style.effects, indent + 1);
    out.push_str(&format!("\n{prefix}}}"));
}

fn write_canvas_path_payload_json(out: &mut String, path: &CanvasPath, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}\"payload\": {{\n"));
    out.push_str(&format!(
        "{field_prefix}\"fill_rule\": {},\n",
        json_string(canvas_fill_rule_name(path.fill_rule))
    ));
    out.push_str(&format!("{field_prefix}\"path\": "));
    write_path_builder_json(out, &path.path, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"fill\": "));
    write_optional_brush_value_json(out, path.fill.as_ref(), indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"stroke\": "));
    write_optional_stroke_json(out, path.stroke.as_ref(), indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"shadow\": "));
    write_optional_shadow_value_json(out, path.shadow.as_ref(), indent + 1);
    out.push_str(&format!("\n{prefix}}}"));
}

fn write_canvas_text_payload_json(out: &mut String, text: &CanvasText, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}\"payload\": {{\n"));
    out.push_str(&format!("{field_prefix}\"frame\": "));
    write_rect_json(out, text.frame, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"content\": "));
    write_text_content_json(out, &text.content, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"text_style\": "));
    write_text_style_json(out, &text.text_style, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"paragraph_style\": "));
    write_paragraph_style_json(out, &text.paragraph_style, indent + 1);
    out.push_str(&format!("\n{prefix}}}"));
}

fn write_canvas_image_payload_json(out: &mut String, image: &CanvasImage, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}\"payload\": {{\n"));
    out.push_str(&format!("{field_prefix}\"frame\": "));
    write_rect_json(out, image.frame, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"source\": "));
    write_media_source_json(out, &image.source, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!(
        "{field_prefix}\"fit\": {},\n",
        json_string(content_fit_name(image.fit))
    ));
    out.push_str(&format!(
        "{field_prefix}\"corner_radius\": {},\n",
        image.corner_radius.get()
    ));
    out.push_str(&format!("{field_prefix}\"source_rect\": "));
    write_optional_rect_json(out, image.source_rect, indent + 1);
    out.push_str(&format!("\n{prefix}}}"));
}

fn write_canvas_group_payload_json(out: &mut String, group: &CanvasGroup, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}\"payload\": {{\n"));
    out.push_str(&format!(
        "{field_prefix}\"mode\": {},\n",
        json_string(canvas_group_mode_name(&group.mode))
    ));
    out.push_str(&format!("{field_prefix}\"shape\": "));
    write_group_shape_json(out, &group.shape, indent + 1);
    out.push_str(",\n");
    out.push_str(&format!("{field_prefix}\"items\": [\n"));
    for (index, child) in group.items.iter().enumerate() {
        write_canvas_scene_item_json(out, child, indent + 2);
        if index + 1 != group.items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{field_prefix}]\n{prefix}}}"));
}

fn write_group_shape_json(out: &mut String, shape: &CanvasGroupShape, indent: usize) {
    match shape {
        CanvasGroupShape::Path { path, fill_rule } => {
            let prefix = "  ".repeat(indent);
            let field_prefix = "  ".repeat(indent + 1);
            out.push_str("{\n");
            out.push_str(&format!("{field_prefix}\"kind\": \"path\",\n"));
            out.push_str(&format!(
                "{field_prefix}\"fill_rule\": {},\n",
                json_string(canvas_fill_rule_name(*fill_rule))
            ));
            out.push_str(&format!("{field_prefix}\"path\": "));
            write_path_builder_json(out, path, indent + 1);
            out.push_str(&format!("\n{prefix}}}"));
        }
    }
}

fn write_path_builder_json(out: &mut String, path: &PathBuilder, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str("{\n");
    out.push_str(&format!(
        "{field_prefix}\"fill_rule\": {},\n",
        json_string(canvas_fill_rule_name(path.fill_rule))
    ));
    out.push_str(&format!("{field_prefix}\"commands\": [\n"));
    for (index, command) in path.commands.iter().enumerate() {
        write_path_command_json(out, command, indent + 2);
        if index + 1 != path.commands.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{field_prefix}]\n{prefix}}}"));
}

fn write_path_command_json(out: &mut String, command: &PathCommand, indent: usize) {
    let prefix = "  ".repeat(indent);
    let field_prefix = "  ".repeat(indent + 1);
    out.push_str(&format!("{prefix}{{\n"));
    match command {
        PathCommand::MoveTo(point_value) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"move_to\",\n"));
            out.push_str(&format!("{field_prefix}\"point\": "));
            write_point_json(out, *point_value, indent + 1);
        }
        PathCommand::LineTo(point_value) => {
            out.push_str(&format!("{field_prefix}\"kind\": \"line_to\",\n"));
            out.push_str(&format!("{field_prefix}\"point\": "));
            write_point_json(out, *point_value, indent + 1);
        }
        PathCommand::QuadTo { ctrl, to } => {
            out.push_str(&format!("{field_prefix}\"kind\": \"quad_to\",\n"));
            out.push_str(&format!("{field_prefix}\"ctrl\": "));
            write_point_json(out, *ctrl, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!("{field_prefix}\"to\": "));
            write_point_json(out, *to, indent + 1);
        }
        PathCommand::CubicTo { ctrl1, ctrl2, to } => {
            out.push_str(&format!("{field_prefix}\"kind\": \"cubic_to\",\n"));
            out.push_str(&format!("{field_prefix}\"ctrl1\": "));
            write_point_json(out, *ctrl1, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!("{field_prefix}\"ctrl2\": "));
            write_point_json(out, *ctrl2, indent + 1);
            out.push_str(",\n");
            out.push_str(&format!("{field_prefix}\"to\": "));
            write_point_json(out, *to, indent + 1);
        }
        PathCommand::Close => {
            out.push_str(&format!("{field_prefix}\"kind\": \"close\"\n"));
            out.push_str(&format!("{prefix}}}"));
            return;
        }
    }
    out.push_str(&format!("\n{prefix}}}"));
}
