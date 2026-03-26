use crate::font;
use crate::graphics::{Canvas2D, DrawCommand};
use crate::ui::node::{ClickHandler, Node, TextNode};
use taffy::prelude::*;

pub(crate) struct Scene {
    pub commands: Vec<DrawCommand>,
    pub hits: Vec<HitRegion>,
}

pub(crate) struct HitRegion {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub on_click: ClickHandler,
}

enum RenderKind {
    None,
    Text {
        content: String,
        text: TextNode,
    },
    Button {
        label_content: String,
        label: TextNode,
        on_click: Option<ClickHandler>,
    },
}

struct RenderTreeNode {
    node_id: NodeId,
    kind: RenderKind,
    children: Vec<RenderTreeNode>,
}

fn measure_text(text: &TextNode, content: &str) -> (f32, f32) {
    font::measure_text(content, &text.style)
}

fn build_render_tree(node: &Node, taffy: &mut TaffyTree<()>) -> RenderTreeNode {
    match node {
        Node::Text(text) => {
            let content = text.source.eval();
            let (w, h) = measure_text(text, &content);
            let style = Style {
                size: Size {
                    width: length(w),
                    height: length(h),
                },
                ..Default::default()
            };
            let id = taffy.new_leaf(style).expect("failed to create text leaf");
            RenderTreeNode {
                node_id: id,
                kind: RenderKind::Text {
                    content,
                    text: text.clone(),
                },
                children: Vec::new(),
            }
        }
        Node::Button(button) => {
            let label_content = button.label.source.eval();
            let (lw, lh) = measure_text(&button.label, &label_content);
            let bw = lw + 24.0;
            let bh = (lh + 12.0).max(30.0);
            let style = Style {
                size: Size {
                    width: length(bw),
                    height: length(bh),
                },
                ..Default::default()
            };
            let id = taffy.new_leaf(style).expect("failed to create button leaf");
            RenderTreeNode {
                node_id: id,
                kind: RenderKind::Button {
                    label_content,
                    label: button.label.clone(),
                    on_click: button.on_click.clone(),
                },
                children: Vec::new(),
            }
        }
        Node::Row(row) => {
            let children: Vec<_> = row
                .children
                .iter()
                .map(|child| build_render_tree(child, taffy))
                .collect();
            let child_ids: Vec<_> = children.iter().map(|n| n.node_id).collect();
            let style = Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                gap: Size {
                    width: length(row.spacing),
                    height: length(0.0),
                },
                ..Default::default()
            };
            let id = taffy
                .new_with_children(style, &child_ids)
                .expect("failed to create row node");
            RenderTreeNode {
                node_id: id,
                kind: RenderKind::None,
                children,
            }
        }
        Node::Column(column) => {
            let children: Vec<_> = column
                .children
                .iter()
                .map(|child| build_render_tree(child, taffy))
                .collect();
            let child_ids: Vec<_> = children.iter().map(|n| n.node_id).collect();
            let style = Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                gap: Size {
                    width: length(0.0),
                    height: length(column.spacing),
                },
                ..Default::default()
            };
            let id = taffy
                .new_with_children(style, &child_ids)
                .expect("failed to create column node");
            RenderTreeNode {
                node_id: id,
                kind: RenderKind::None,
                children,
            }
        }
    }
}

fn emit_from_layout(
    render: &RenderTreeNode,
    taffy: &TaffyTree<()>,
    canvas: &mut Canvas2D,
    hits: &mut Vec<HitRegion>,
    abs_x: f32,
    abs_y: f32,
) {
    let layout = taffy.layout(render.node_id).expect("failed to get layout");
    let x = abs_x + layout.location.x;
    let y = abs_y + layout.location.y;
    let w = layout.size.width;
    let h = layout.size.height;

    match &render.kind {
        RenderKind::None => {}
        RenderKind::Text { content, text } => {
            canvas.set_text_style(text.style.clone());
            canvas.fill_text(content.clone(), x, y);
        }
        RenderKind::Button {
            label_content,
            label,
            on_click,
        } => {
            canvas.set_fill_style([0.2, 0.52, 0.9, 1.0]);
            canvas.fill_rect(x, y, w, h);

            let (lw, lh) = font::measure_text(label_content, &label.style);
            let label_x = x + (w - lw) * 0.5;
            let label_y = y + (h - lh) * 0.5;
            canvas.set_text_style(label.style.clone());
            canvas.fill_text(label_content.clone(), label_x, label_y);

            if let Some(handler) = on_click {
                hits.push(HitRegion {
                    x,
                    y,
                    w,
                    h,
                    on_click: handler.clone(),
                });
            }
        }
    }

    for child in &render.children {
        emit_from_layout(child, taffy, canvas, hits, x, y);
    }
}

pub(crate) fn build_scene(root: &Node, width: u32, height: u32) -> Scene {
    let mut taffy = TaffyTree::<()>::new();
    let render_root = build_render_tree(root, &mut taffy);

    let root_centered = matches!(root, Node::Column(c) if c.centered);
    let viewport_style = if root_centered {
        Style {
            display: Display::Flex,
            size: Size {
                width: length(width as f32),
                height: length(height as f32),
            },
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            ..Default::default()
        }
    } else {
        Style {
            display: Display::Flex,
            size: Size {
                width: length(width as f32),
                height: length(height as f32),
            },
            padding: Rect {
                left: length(20.0),
                right: length(20.0),
                top: length(20.0),
                bottom: length(20.0),
            },
            ..Default::default()
        }
    };

    let viewport = taffy
        .new_with_children(viewport_style, &[render_root.node_id])
        .expect("failed to create viewport node");

    taffy
        .compute_layout(
            viewport,
            Size {
                width: AvailableSpace::Definite(width as f32),
                height: AvailableSpace::Definite(height as f32),
            },
        )
        .expect("failed to compute taffy layout");

    let viewport_layout = taffy.layout(viewport).expect("failed to get viewport layout");

    let mut canvas = Canvas2D::new();
    let mut hits = Vec::new();
    emit_from_layout(
        &render_root,
        &taffy,
        &mut canvas,
        &mut hits,
        viewport_layout.location.x,
        viewport_layout.location.y,
    );

    Scene {
        commands: canvas.into_commands(),
        hits,
    }
}
