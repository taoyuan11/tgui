use super::*;

use crate::ui::widget::{
    ItemLayout, Tree, TreeCheckState, TreeNode, TreeSelectionMode, TreeStyle, WidgetKey,
};

fn sample_nodes() -> Vec<TreeNode<&'static str>> {
    vec![
        TreeNode::keyed("root", "Root").children([
            TreeNode::keyed("child-a", "Child A").child(TreeNode::keyed("leaf", "Leaf")),
            TreeNode::keyed("child-b", "Child B").disable(true),
        ]),
        TreeNode::keyed("sibling", "Sibling"),
    ]
}

fn tree_layout(tree: WidgetTree<()>, viewport: Rect) -> crate::ui::widget::ResolvedSceneLayout<()> {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    )
}

#[test]
fn tree_flattens_only_expanded_visible_nodes() {
    let tree: WidgetTree<()> = WidgetTree::new(
        Tree::<&'static str, ()>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(vec![WidgetKey::from("root")])
            .size(dp(240.0), dp(180.0)),
    );

    let layout = tree_layout(tree, Rect::new(0.0, 0.0, 240.0, 180.0));
    let ResolvedWidgetKind::Virtual { children, .. } = &layout.resolved_root.kind else {
        panic!("Tree should resolve to the Virtual widget path");
    };
    let keys = children
        .iter()
        .filter_map(|child| child.tree_node.as_ref().map(|state| state.key.clone()))
        .collect::<Vec<_>>();

    assert_eq!(
        keys,
        vec![
            WidgetKey::from("root"),
            WidgetKey::from("child-a"),
            WidgetKey::from("child-b"),
            WidgetKey::from("sibling"),
        ]
    );
    assert_eq!(children[1].tree_node.as_ref().unwrap().depth, 1);
    assert!(children[1].tree_node.as_ref().unwrap().has_children);
    assert!(children[2].tree_node.as_ref().unwrap().disabled.resolve());
}

#[test]
fn tree_checkbox_state_cascades_over_enabled_descendants() {
    let tree: WidgetTree<()> = WidgetTree::new(
        Tree::<&'static str, ()>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(vec![WidgetKey::from("root")])
            .checkable(true)
            .checked_keys(vec![WidgetKey::from("child-a"), WidgetKey::from("leaf")])
            .size(dp(240.0), dp(180.0)),
    );

    let layout = tree_layout(tree, Rect::new(0.0, 0.0, 240.0, 180.0));
    let ResolvedWidgetKind::Virtual { children, .. } = &layout.resolved_root.kind else {
        panic!("Tree should resolve to the Virtual widget path");
    };
    let root = children[0].tree_node.as_ref().unwrap();
    let child_a = children[1].tree_node.as_ref().unwrap();
    let child_b = children[2].tree_node.as_ref().unwrap();

    assert_eq!(root.check_state, TreeCheckState::Indeterminate);
    assert_eq!(child_a.check_state, TreeCheckState::Checked);
    assert_eq!(child_b.check_state, TreeCheckState::Unchecked);
    assert_eq!(
        root.check_target_keys.as_ref(),
        &[
            WidgetKey::from("root"),
            WidgetKey::from("child-a"),
            WidgetKey::from("leaf")
        ]
    );
}

#[test]
fn tree_selected_row_emits_selected_background() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let selected = Color::hexa(0xAA5500FF);
    let tree: WidgetTree<()> = WidgetTree::new(
        Tree::<&'static str, ()>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(vec![WidgetKey::from("root")])
            .selection_mode(TreeSelectionMode::Multiple)
            .selected_keys(vec![WidgetKey::from("child-a")])
            .style_full(move |ctx| {
                let mut style = TreeStyle::default_for_theme(ctx.theme);
                style.item_selected_background = selected.into();
                style
            })
            .size(dp(240.0), dp(180.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 180.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(
        rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.stroke_width == 0.0 && shape.color == selected),
        "selected Tree row should render its selected background"
    );
}

#[test]
fn tree_chrome_uses_svg_textures_for_disclosure_and_checks() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Tree::<&'static str, ()>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(vec![WidgetKey::from("root")])
            .checkable(true)
            .checked_keys(vec![WidgetKey::from("child-a"), WidgetKey::from("leaf")])
            .size(dp(240.0), dp(180.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 180.0),
        None,
        None,
        None,
        None,
        false,
    );
    let texts = rendered
        .primitives
        .texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();

    assert!(!texts.contains(&"keyboard_arrow_right"));
    assert!(!texts.contains(&"check_box"));
    assert!(!texts.contains(&"check_box_outline_blank"));
    assert!(!texts.contains(&"indeterminate_check_box"));
    assert!(!texts
        .iter()
        .any(|text| matches!(*text, ">" | "v" | "[x]" | "[ ]" | "[-]")));
    assert!(
        rendered.primitives.textures.len() >= 5,
        "Tree chrome icons should render as SVG textures"
    );
}

#[test]
fn tree_full_width_row_content_does_not_overlap_checkbox_chrome() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Tree::<&'static str, ()>::new(sample_nodes(), |ctx| {
            Text::new(ctx.item).width(pct(100.0)).into()
        })
        .expanded_keys(vec![WidgetKey::from("root")])
        .checkable(true)
        .size(dp(240.0), dp(180.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 180.0),
        None,
        None,
        None,
        None,
        false,
    );
    let label = rendered
        .primitives
        .texts
        .iter()
        .find(|text| text.content.as_ref() == "Root")
        .expect("root label should render");
    let chrome_right = rendered
        .primitives
        .textures
        .iter()
        .filter(|texture| {
            texture.frame.y < label.frame.bottom() && texture.frame.bottom() > label.frame.y
        })
        .map(|texture| texture.frame.right().get())
        .fold(0.0_f32, f32::max);
    let checkbox = rendered
        .primitives
        .textures
        .iter()
        .filter(|texture| {
            texture.frame.y < label.frame.bottom() && texture.frame.bottom() > label.frame.y
        })
        .max_by(|left, right| {
            left.frame
                .x
                .get()
                .partial_cmp(&right.frame.x.get())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("root checkbox chrome should render");
    let label_center_y = label.frame.y.get() + label.frame.height.get() * 0.5;
    let checkbox_center_y = checkbox.frame.y.get() + checkbox.frame.height.get() * 0.5;

    assert!(
        label.frame.x.get() >= chrome_right - 0.5,
        "Tree row label should start after checkbox/disclosure chrome; label={:?}, chrome_right={chrome_right}",
        label.frame
    );
    assert!(
        (label_center_y - checkbox_center_y).abs() <= 0.5,
        "Tree row label and checkbox should be vertically centered together; label={:?}, checkbox={:?}",
        label.frame,
        checkbox.frame
    );
}

#[test]
fn tree_expanded_disclosure_icon_is_rotated() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Tree::<&'static str, ()>::new(sample_nodes(), |ctx| Text::new(ctx.item).into())
            .expanded_keys(vec![WidgetKey::from("root")])
            .size(dp(240.0), dp(180.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 180.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(
        rendered
            .primitives
            .textures
            .iter()
            .any(|texture| texture.quad.is_some()),
        "expanded Tree disclosure should keep its rotation as a texture quad"
    );
}

#[test]
fn tree_virtualizes_large_expanded_roots() {
    let nodes = (0..1_000)
        .map(|index| TreeNode::keyed(index, format!("Node {index}")))
        .collect::<Vec<_>>();
    let tree: WidgetTree<()> = WidgetTree::new(
        Tree::<String, ()>::new(nodes, |ctx| Text::new(ctx.item).into())
            .item_layout(ItemLayout::Fixed {
                item_extent: dp(32.0),
                spacing: dp(2.0),
                overscan: 1,
            })
            .size(dp(240.0), dp(120.0)),
    );

    let layout = tree_layout(tree, Rect::new(0.0, 0.0, 240.0, 120.0));
    let ResolvedWidgetKind::Virtual {
        children,
        window_plan,
        ..
    } = &layout.resolved_root.kind
    else {
        panic!("Tree should resolve to the Virtual widget path");
    };

    assert!(children.len() < 10);
    assert_eq!(children.len(), window_plan.placements.len());
    assert!(children.iter().all(|child| child.tree_node.is_some()));
}
