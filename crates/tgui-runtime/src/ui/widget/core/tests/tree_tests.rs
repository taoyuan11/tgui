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

#[test]
fn tree_default_style_follows_theme_density() {
    let expected = [
        (
            crate::ui::theme::Density::Compact,
            dp(32.0),
            dp(6.0),
            dp(4.0),
            dp(6.0),
            dp(16.0),
            dp(20.0),
            sp(16.0),
        ),
        (
            crate::ui::theme::Density::Comfortable,
            dp(40.0),
            dp(8.0),
            dp(8.0),
            dp(8.0),
            dp(20.0),
            dp(24.0),
            sp(18.0),
        ),
        (
            crate::ui::theme::Density::Spacious,
            dp(48.0),
            dp(12.0),
            dp(12.0),
            dp(12.0),
            dp(24.0),
            dp(28.0),
            sp(20.0),
        ),
    ];

    for (density, height, padding_x, padding_y, radius, indent, chrome, icon) in expected {
        let mut theme = Theme::light();
        theme.density = density;
        let style = TreeStyle::default_for_theme(&theme);

        assert_eq!(style.item_height, height);
        assert_eq!(style.item_padding.left, padding_x);
        assert_eq!(style.item_padding.top, padding_y);
        assert_eq!(style.item_radius, radius);
        assert_eq!(style.indent_width, indent);
        assert_eq!(style.disclosure_width, chrome);
        assert_eq!(style.checkbox_width, chrome);
        assert_eq!(style.disclosure_icon_size, icon);
        assert_eq!(style.checkbox_icon_size, icon);
    }
}

#[test]
fn tree_loading_signal_switches_slots_on_the_existing_tree() {
    let context = test_context();
    let loading = context.state(true);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 240.0, 120.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        Tree::<&'static str, ()>::new(vec![TreeNode::keyed("root", "Root")], |ctx| {
            Text::new(ctx.item).into()
        })
        .loading(loading.signal())
        .loading_view(Text::new("loading"))
        .size(dp(240.0), dp(120.0)),
    );
    let render_labels = |animations: &mut AnimationEngine| {
        tree.render_output(
            &font_manager,
            &theme,
            &media,
            animations,
            None,
            None,
            &HashMap::new(),
            viewport,
            None,
            None,
            None,
            None,
            false,
        )
        .primitives
        .texts
        .iter()
        .map(|text| text.content.to_string())
        .collect::<Vec<_>>()
    };

    let first = render_labels(&mut animations);
    assert!(first.iter().any(|label| label == "loading"));
    assert!(!first.iter().any(|label| label == "Root"));
    let loading_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let ResolvedWidgetKind::Container {
        children: loading_children,
        ..
    } = &loading_layout.resolved_root.kind
    else {
        panic!("reactive Tree slots should resolve through a retained container");
    };
    assert_eq!(loading_children.len(), 1);
    assert!(loading_children[0].tree_root.is_none());
    assert!(!matches!(
        loading_children[0].kind,
        ResolvedWidgetKind::Virtual { .. }
    ));

    loading.set(false);
    let loaded = render_labels(&mut animations);
    assert!(loaded.iter().any(|label| label == "Root"));
    assert!(!loaded.iter().any(|label| label == "loading"));
    let loaded_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let ResolvedWidgetKind::Container {
        children: loaded_children,
        ..
    } = &loaded_layout.resolved_root.kind
    else {
        panic!("reactive Tree should keep its retained container");
    };
    assert_eq!(loaded_children.len(), 1);
    assert_eq!(
        loaded_children[0]
            .tree_root
            .as_ref()
            .expect("loaded Tree root metadata")
            .node_count,
        1
    );

    loading.set(true);
    let loading_again = render_labels(&mut animations);
    assert!(loading_again.iter().any(|label| label == "loading"));
    assert!(!loading_again.iter().any(|label| label == "Root"));
}

#[test]
fn tree_loading_signal_prioritizes_loading_over_empty_on_the_existing_tree() {
    let context = test_context();
    let loading = context.state(false);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 240.0, 220.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        Tree::<&'static str, ()>::new(Vec::<TreeNode<&'static str>>::new(), |ctx| {
            Text::new(ctx.item).into()
        })
        .loading(loading.signal())
        .loading_view(
            Stack::new()
                .child(Text::new("loading"))
                .size(dp(240.0), dp(160.0)),
        )
        .empty(
            Stack::new()
                .child(Text::new("empty"))
                .size(dp(240.0), dp(150.0)),
        )
        .size(dp(240.0), dp(220.0)),
    );
    let render_labels = |animations: &mut AnimationEngine| {
        tree.render_output(
            &font_manager,
            &theme,
            &media,
            animations,
            None,
            None,
            &HashMap::new(),
            viewport,
            None,
            None,
            None,
            None,
            false,
        )
        .primitives
        .texts
        .iter()
        .map(|text| text.content.to_string())
        .collect::<Vec<_>>()
    };

    let empty = render_labels(&mut animations);
    assert!(empty.iter().any(|label| label == "empty"));
    assert!(!empty.iter().any(|label| label == "loading"));
    let empty_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let ResolvedWidgetKind::Container {
        children: empty_children,
        ..
    } = &empty_layout.resolved_root.kind
    else {
        panic!("reactive Tree slots should resolve through a retained container");
    };
    assert_eq!(empty_children.len(), 1);
    assert_eq!(
        empty_children[0]
            .layout
            .height
            .as_ref()
            .map(|height| height.resolve()),
        Some(crate::ui::layout::Length::Px(dp(150.0)))
    );
    assert!(empty_children[0].tree_root.is_none());

    loading.set(true);
    let loading_labels = render_labels(&mut animations);
    assert!(loading_labels.iter().any(|label| label == "loading"));
    assert!(!loading_labels.iter().any(|label| label == "empty"));
    let loading_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let ResolvedWidgetKind::Container {
        children: loading_children,
        ..
    } = &loading_layout.resolved_root.kind
    else {
        panic!("reactive Tree slots should keep their retained container");
    };
    assert_eq!(loading_children.len(), 1);
    assert_eq!(
        loading_children[0]
            .layout
            .height
            .as_ref()
            .map(|height| height.resolve()),
        Some(crate::ui::layout::Length::Px(dp(160.0)))
    );
    assert!(loading_children[0].tree_root.is_none());

    loading.set(false);
    let empty_again = render_labels(&mut animations);
    assert!(empty_again.iter().any(|label| label == "empty"));
    assert!(!empty_again.iter().any(|label| label == "loading"));
}

#[test]
fn tree_expanded_checked_and_checkable_signals_update_on_the_same_tree() {
    let context = test_context();
    let expanded = context.state(Vec::<WidgetKey>::new());
    let checked = context.state(Vec::<WidgetKey>::new());
    let checkable = context.state(false);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 240.0, 120.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        Tree::<&'static str, ()>::new(
            vec![TreeNode::keyed("root", "Root").child(TreeNode::keyed("child", "Child"))],
            |ctx| Text::new(ctx.item).into(),
        )
        .expanded_keys(expanded.signal())
        .checked_keys(checked.signal())
        .checkable(checkable.signal())
        .size(dp(240.0), dp(120.0)),
    );

    let initial_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let ResolvedWidgetKind::Virtual {
        children: initial_rows,
        ..
    } = &initial_layout.resolved_root.kind
    else {
        panic!("Tree should resolve to a virtual widget");
    };
    assert_eq!(initial_rows.len(), 1);
    let initial_root = initial_rows[0]
        .tree_node
        .as_ref()
        .expect("root tree node state");
    assert!(!initial_root.expanded);
    assert!(!initial_root.checkable.resolve());
    assert_eq!(initial_root.check_state, TreeCheckState::Unchecked);
    let initial_root_id = initial_rows[0].id;
    let initial_scene = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    let initial_label_x = initial_scene
        .primitives
        .texts
        .iter()
        .find(|text| text.content.as_ref() == "Root")
        .expect("root label should render")
        .frame
        .x;

    checkable.set(true);
    expanded.set(vec![WidgetKey::from("root")]);
    checked.set(vec![WidgetKey::from("root"), WidgetKey::from("child")]);
    let updated_layout = tree.build_scene_layout_at_with_previous(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        Instant::now(),
        Some(&initial_layout),
    );
    let ResolvedWidgetKind::Virtual {
        children: updated_rows,
        ..
    } = &updated_layout.resolved_root.kind
    else {
        panic!("Tree should remain a virtual widget");
    };
    assert_eq!(updated_rows.len(), 2);
    assert_eq!(updated_rows[0].id, initial_root_id);
    let updated_root = updated_rows[0]
        .tree_node
        .as_ref()
        .expect("updated root tree node state");
    assert!(updated_root.expanded);
    assert!(updated_root.checkable.resolve());
    assert_eq!(updated_root.check_state, TreeCheckState::Checked);
    assert_eq!(
        updated_rows[1]
            .tree_node
            .as_ref()
            .expect("expanded child state")
            .key,
        WidgetKey::from("child")
    );
    let updated_scene = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    let updated_label_x = updated_scene
        .primitives
        .texts
        .iter()
        .find(|text| text.content.as_ref() == "Root")
        .expect("updated root label should render")
        .frame
        .x;
    assert_eq!(
        updated_label_x - initial_label_x,
        TreeStyle::default_for_theme(&theme).checkbox_width
    );

    checked.set(vec![WidgetKey::from("child")]);
    let partial_layout = tree.build_scene_layout_at_with_previous(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        Instant::now(),
        Some(&updated_layout),
    );
    let ResolvedWidgetKind::Virtual {
        children: partial_rows,
        ..
    } = &partial_layout.resolved_root.kind
    else {
        panic!("Tree should remain a virtual widget");
    };
    assert_eq!(
        partial_rows[0]
            .tree_node
            .as_ref()
            .expect("partially checked root state")
            .check_state,
        TreeCheckState::Indeterminate
    );
}

#[test]
fn compact_tree_selected_scene_uses_density_geometry() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut theme = Theme::light();
    theme.density = crate::ui::theme::Density::Compact;
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Tree::<&'static str, ()>::new(vec![TreeNode::keyed("root", "Root")], |ctx| {
            Text::new(ctx.item).into()
        })
        .selected_keys(vec![WidgetKey::from("root")])
        .item_layout(ItemLayout::Fixed {
            item_extent: dp(32.0),
            spacing: Dp::ZERO,
            overscan: 0,
        })
        .size(dp(240.0), dp(64.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 64.0),
        None,
        None,
        None,
        None,
        false,
    );
    let style = TreeStyle::default_for_theme(&theme);
    let selected = style.item_selected_background.resolve();
    let state_layer = rendered
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.color == selected)
        .expect("selected Tree row should emit its state layer");
    let label = rendered
        .primitives
        .texts
        .iter()
        .find(|text| text.content.as_ref() == "Root")
        .expect("Tree node label should render");

    assert_eq!(state_layer.rect.height, style.item_height);
    assert_eq!(state_layer.corner_radius, style.item_radius.get());
    assert_eq!(
        label.frame.x - state_layer.rect.x,
        style.item_padding.left + style.disclosure_width
    );
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

#[test]
fn large_tree_selection_snapshot_is_shared_and_membership_stays_equivalent() {
    let nodes = (0..100_000)
        .map(|index| TreeNode::keyed(index, format!("Node {index}")))
        .collect::<Vec<_>>();
    let selected = (0..100_000)
        .step_by(2)
        .map(WidgetKey::from)
        .collect::<Vec<_>>();
    let checked = selected.clone();
    let tree: WidgetTree<()> = WidgetTree::new(
        Tree::<String, ()>::new(nodes, |ctx| Text::new(ctx.item).into())
            .selected_keys(selected)
            .checked_keys(checked)
            .checkable(true)
            .selection_mode(TreeSelectionMode::Multiple)
            .item_layout(ItemLayout::Fixed {
                item_extent: dp(40.0),
                spacing: Dp::ZERO,
                overscan: 0,
            })
            .size(dp(240.0), dp(160.0)),
    );

    let layout = tree_layout(tree, Rect::new(0.0, 0.0, 240.0, 160.0));
    let root_selection = layout
        .resolved_root
        .tree_root
        .as_ref()
        .expect("Tree root selection metadata")
        .selection
        .clone();
    let ResolvedWidgetKind::Virtual { children, .. } = &layout.resolved_root.kind else {
        panic!("Tree should resolve to a virtual widget");
    };
    let states = children
        .iter()
        .map(|child| child.tree_node.as_ref().expect("visible tree row state"))
        .collect::<Vec<_>>();
    let shared_keys = root_selection.selected_keys.resolve();
    let shared_membership = root_selection.selected_key_membership.resolve();
    let controlled_keys = states[0].controlled_keys.clone();
    let shared_checked = controlled_keys.checked.resolve();
    for state in states {
        assert!(Arc::ptr_eq(&root_selection, &state.selection));
        assert!(Arc::ptr_eq(&controlled_keys, &state.controlled_keys));
        let keys = state.selection.selected_keys.resolve();
        let membership = state.selection.selected_key_membership.resolve();
        assert!(Arc::ptr_eq(&shared_keys, &keys));
        assert!(Arc::ptr_eq(&shared_membership, &membership));
        assert_eq!(
            membership.contains(&state.key),
            keys.contains(&state.key),
            "membership snapshot must preserve controlled Tree selection semantics"
        );
        let checked = state.controlled_keys.checked.resolve();
        assert!(Arc::ptr_eq(&shared_checked, &checked));
        assert_eq!(
            checked.membership.contains(&state.key),
            checked.ordered.contains(&state.key),
            "checked membership must share the ordered controlled snapshot"
        );
    }
}

#[test]
#[cfg(target_pointer_width = "64")]
fn tree_selection_metadata_stays_out_of_line() {
    let size = std::mem::size_of::<crate::ui::widget::common::TreeNodeState<()>>();
    let metadata_size = std::mem::size_of::<crate::ui::widget::common::TreeSelectionMetadata>();
    assert!(
        size <= 2_900,
        "Tree selection lookup tables must remain behind Arc; got {size} bytes"
    );
    assert!(
        metadata_size <= 352,
        "shared Tree selection metadata grew to {metadata_size} bytes"
    );
}
