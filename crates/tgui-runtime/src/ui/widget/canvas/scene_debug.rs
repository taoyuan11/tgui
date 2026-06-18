use super::*;

mod json;

pub(crate) use self::json::{
    canvas_text_horizontal_align_name, canvas_text_overflow_name, canvas_text_vertical_align_name,
    canvas_text_wrap_name, export_canvas_scene_json,
};
use self::json::{write_debug_node_json, write_debug_stats_json};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasSceneDebugStats {
    pub root_items: usize,
    pub total_items: usize,
    pub named_items: usize,
    pub visible_items: usize,
    pub hit_testable_items: usize,
    pub path_items: usize,
    pub text_items: usize,
    pub image_items: usize,
    pub group_items: usize,
    pub max_depth: usize,
    pub bounds: Option<Rect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSceneDebugNode {
    pub id: CanvasItemId,
    pub name: Option<String>,
    pub kind: CanvasItemKind,
    pub depth: usize,
    pub index_path: Vec<usize>,
    pub visible: bool,
    pub hit_test: bool,
    pub opacity: f32,
    pub blend_mode: CanvasBlendMode,
    pub bounds: Option<Rect>,
    pub child_count: usize,
    pub summary: String,
    pub children: Vec<CanvasSceneDebugNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSceneDebugInfo {
    pub stats: CanvasSceneDebugStats,
    pub nodes: Vec<CanvasSceneDebugNode>,
}

impl CanvasSceneDebugInfo {
    pub fn to_pretty_text(&self) -> String {
        let mut out = String::new();
        out.push_str("CanvasScene\n");
        out.push_str(&format!(
            "  root_items={} total_items={} named_items={} visible_items={} hit_testable_items={} max_depth={}\n",
            self.stats.root_items,
            self.stats.total_items,
            self.stats.named_items,
            self.stats.visible_items,
            self.stats.hit_testable_items,
            self.stats.max_depth,
        ));
        out.push_str(&format!(
            "  kinds: path={} text={} image={} group={}\n",
            self.stats.path_items,
            self.stats.text_items,
            self.stats.image_items,
            self.stats.group_items,
        ));
        if let Some(bounds) = self.stats.bounds {
            out.push_str(&format!(
                "  bounds: x={:.1} y={:.1} width={:.1} height={:.1}\n",
                bounds.x.get(),
                bounds.y.get(),
                bounds.width.get(),
                bounds.height.get(),
            ));
        }
        for node in &self.nodes {
            write_debug_node_text(&mut out, node);
        }
        out
    }

    pub fn to_pretty_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str("  \"stats\": ");
        write_debug_stats_json(&mut out, &self.stats, 1);
        out.push_str(",\n  \"nodes\": [\n");
        for (index, node) in self.nodes.iter().enumerate() {
            write_debug_node_json(&mut out, node, 2);
            if index + 1 != self.nodes.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  ]\n}");
        out
    }
}

pub(crate) fn build_canvas_scene_debug_info(scene: &CanvasScene) -> CanvasSceneDebugInfo {
    let mut stats = CanvasSceneDebugStats {
        root_items: scene.items().len(),
        bounds: scene.bounds(),
        ..Default::default()
    };
    let nodes = scene
        .items()
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let mut path = vec![index];
            build_debug_node(item, 0, &mut path, &mut stats)
        })
        .collect();
    CanvasSceneDebugInfo { stats, nodes }
}

fn build_debug_node(
    item: &CanvasItem,
    depth: usize,
    index_path: &mut Vec<usize>,
    stats: &mut CanvasSceneDebugStats,
) -> CanvasSceneDebugNode {
    stats.total_items += 1;
    stats.max_depth = stats.max_depth.max(depth);
    if item.name().is_some() {
        stats.named_items += 1;
    }
    if item.style().visible {
        stats.visible_items += 1;
    }
    if item.style().hit_test {
        stats.hit_testable_items += 1;
    }
    match item.kind() {
        CanvasItemKind::Path => stats.path_items += 1,
        CanvasItemKind::Text => stats.text_items += 1,
        CanvasItemKind::Image => stats.image_items += 1,
        CanvasItemKind::Group => stats.group_items += 1,
    }

    let summary = match item {
        CanvasItem::Path(path) => format!(
            "path(fill={}, stroke={}, shadow={})",
            path.fill.is_some(),
            path.stroke.is_some(),
            path.shadow.is_some()
        ),
        CanvasItem::Text(text) => format!("text(chars={})", text.plain_text().chars().count()),
        CanvasItem::Image(image) => format!("image(fit={:?})", image.fit),
        CanvasItem::Group(group) => {
            format!("group(mode={:?}, items={})", group.mode, group.items.len())
        }
    };

    let children = match item {
        CanvasItem::Group(group) => group
            .items
            .iter()
            .enumerate()
            .map(|(index, child)| {
                index_path.push(index);
                let node = build_debug_node(child, depth + 1, index_path, stats);
                index_path.pop();
                node
            })
            .collect(),
        _ => Vec::new(),
    };

    CanvasSceneDebugNode {
        id: item.id(),
        name: item.name().map(ToOwned::to_owned),
        kind: item.kind(),
        depth,
        index_path: index_path.clone(),
        visible: item.style().visible,
        hit_test: item.style().hit_test,
        opacity: item.style().opacity,
        blend_mode: item.style().blend_mode,
        bounds: item.layout_bounds().map(rect_from_bounds),
        child_count: item.children().len(),
        summary,
        children,
    }
}

fn write_debug_node_text(out: &mut String, node: &CanvasSceneDebugNode) {
    let indent = "  ".repeat(node.depth + 1);
    out.push_str(&format!(
        "{}- {:?} id={}{} visible={} hit_test={} opacity={:.2}",
        indent,
        node.kind,
        node.id.get(),
        node.name
            .as_ref()
            .map(|name| format!(" name=\"{}\"", name))
            .unwrap_or_default(),
        node.visible,
        node.hit_test,
        node.opacity,
    ));
    if let Some(bounds) = node.bounds {
        out.push_str(&format!(
            " bounds=({:.1}, {:.1}, {:.1}, {:.1})",
            bounds.x.get(),
            bounds.y.get(),
            bounds.width.get(),
            bounds.height.get(),
        ));
    }
    out.push_str(&format!(" {}\n", node.summary));
    for child in &node.children {
        write_debug_node_text(out, child);
    }
}
