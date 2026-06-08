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
fn tree_chrome_uses_material_icons_for_disclosure_and_checks() {
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

    assert!(texts.contains(&"keyboard_arrow_right"));
    assert!(texts.contains(&"check_box"));
    assert!(texts.contains(&"check_box_outline_blank"));
    assert!(texts.contains(&"indeterminate_check_box"));
    assert!(!texts
        .iter()
        .any(|text| matches!(*text, ">" | "v" | "[x]" | "[ ]" | "[-]")));
    assert!(
        rendered
            .primitives
            .texts
            .iter()
            .filter(|text| {
                matches!(
                    text.content.as_ref(),
                    "keyboard_arrow_right"
                        | "check_box"
                        | "check_box_outline_blank"
                        | "indeterminate_check_box"
                )
            })
            .all(|text| text.font_family.is_some()),
        "Tree chrome icons should resolve through the bundled icon font"
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
            .texts
            .iter()
            .any(|text| { text.content.as_ref() == "keyboard_arrow_right" && text.quad.is_some() }),
        "expanded Tree disclosure should render as an animated rotated Material icon"
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
