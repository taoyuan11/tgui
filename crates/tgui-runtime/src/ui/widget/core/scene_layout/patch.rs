use super::*;

pub(super) fn collect_indexes<VM>(
    node: &ResolvedElement<VM>,
    parent: Option<WidgetId>,
    depth: usize,
    path: &mut Vec<usize>,
    paths: &mut HashMap<WidgetId, Vec<usize>>,
    parents: &mut HashMap<WidgetId, Option<WidgetId>>,
    depths: &mut HashMap<WidgetId, usize>,
) {
    paths.insert(node.id, path.clone());
    parents.insert(node.id, parent);
    depths.insert(node.id, depth);
    if let ResolvedWidgetKind::Container { children, .. }
    | ResolvedWidgetKind::Virtual { children, .. } = &node.kind
    {
        for (index, child) in children.iter().enumerate() {
            path.push(index);
            collect_indexes(
                child,
                Some(node.id),
                depth + 1,
                path,
                paths,
                parents,
                depths,
            );
            path.pop();
        }
    }
}

pub(super) fn collect_resolved_widget_ids<VM>(node: &ResolvedElement<VM>, ids: &mut Vec<WidgetId>) {
    ids.push(node.id);
    if let ResolvedWidgetKind::Container { children, .. }
    | ResolvedWidgetKind::Virtual { children, .. } = &node.kind
    {
        for child in children {
            collect_resolved_widget_ids(child, ids);
        }
    }
}

pub(super) fn patch_layout_at_path<VM>(
    current: &mut ResolvedElement<VM>,
    layout_node: &mut LayoutNode,
    path: &[usize],
    next: ResolvedElement<VM>,
    taffy: &mut TaffyTree<MeasureContext>,
    animations: &mut AnimationEngine,
    theme: &Theme,
    units: UnitContext,
    viewport: Rect,
    now: std::time::Instant,
    parent_kind: Option<ContainerKind>,
    is_root: bool,
) -> Result<(), taffy::TaffyError> {
    if path.is_empty() {
        *current = patch_layout_tree(
            current,
            next,
            layout_node,
            taffy,
            animations,
            theme,
            units,
            parent_kind,
            viewport,
            now,
            is_root,
        )?;
        return Ok(());
    }

    let (parent_kind, children) = match &mut current.kind {
        ResolvedWidgetKind::Container {
            layout, children, ..
        } => (Some(layout.kind.clone()), children),
        ResolvedWidgetKind::Virtual { children, .. } => (None, children),
        _ => return Ok(()),
    };
    let child_index = path[0];
    patch_layout_at_path(
        &mut children[child_index],
        &mut layout_node.children[child_index],
        &path[1..],
        next,
        taffy,
        animations,
        theme,
        units,
        viewport,
        now,
        parent_kind,
        false,
    )
}

fn patch_layout_tree<VM>(
    current: &mut ResolvedElement<VM>,
    mut next: ResolvedElement<VM>,
    layout_node: &mut LayoutNode,
    taffy: &mut TaffyTree<MeasureContext>,
    animations: &mut AnimationEngine,
    theme: &Theme,
    units: UnitContext,
    parent_kind: Option<ContainerKind>,
    viewport: Rect,
    now: std::time::Instant,
    is_root: bool,
) -> Result<ResolvedElement<VM>, taffy::TaffyError> {
    let owner = next.id.dependency_owner(DependencyPhase::Layout);
    track_dependency_scope(owner, || {
        let next_parent_kind = match &next.kind {
            ResolvedWidgetKind::Container { layout, .. } => Some(layout.kind.clone()),
            ResolvedWidgetKind::Virtual { .. } => None,
            _ => None,
        };

        let old_children = match std::mem::replace(
            &mut current.kind,
            ResolvedWidgetKind::Container {
                layout: ContainerLayout::flow(),
                children: Vec::new(),
                runtime_style: {
                    let theme = Theme::default();
                    ResolvedRuntimeSurfaceStyle {
                        base: crate::ui::widget::style::ContainerStyle::default_for_theme(&theme),
                        local: None,
                        explicit_visual: VisualStyle::default(),
                        explicit_background: None,
                    }
                },
            },
        ) {
            ResolvedWidgetKind::Container { children, .. } => children,
            ResolvedWidgetKind::Virtual { children, .. } => children,
            other => {
                current.kind = other;
                Vec::new()
            }
        };
        let old_layout_children = std::mem::take(&mut layout_node.children);
        let mut old_children_by_id: HashMap<_, _> = old_children
            .into_iter()
            .zip(old_layout_children)
            .map(|(child, layout)| (child.id, (child, layout)))
            .collect();

        let next_children = match &mut next.kind {
            ResolvedWidgetKind::Container { children, .. } => std::mem::take(children),
            ResolvedWidgetKind::Virtual { children, .. } => std::mem::take(children),
            _ => Vec::new(),
        };

        let mut patched_children = Vec::with_capacity(next_children.len());
        let mut patched_layout_children = Vec::with_capacity(next_children.len());

        for child in next_children {
            if let Some((mut existing_child, mut existing_layout)) =
                old_children_by_id.remove(&child.id)
            {
                let patched_child = patch_layout_tree(
                    &mut existing_child,
                    child,
                    &mut existing_layout,
                    taffy,
                    animations,
                    theme,
                    units,
                    next_parent_kind.clone(),
                    viewport,
                    now,
                    false,
                )?;
                patched_children.push(patched_child);
                patched_layout_children.push(existing_layout);
            } else {
                let new_layout = child.build_layout_tree(
                    taffy,
                    animations,
                    theme,
                    units,
                    next_parent_kind.clone(),
                    viewport,
                    false,
                    now,
                )?;
                patched_layout_children.push(new_layout);
                patched_children.push(child);
            }
        }

        for (_, (_, stale_layout)) in old_children_by_id {
            remove_layout_subtree(taffy, &stale_layout)?;
        }

        match &mut next.kind {
            ResolvedWidgetKind::Container { children, .. }
            | ResolvedWidgetKind::Virtual { children, .. } => {
                *children = patched_children;
            }
            _ => {}
        }

        taffy.set_style(
            layout_node.node,
            next.taffy_style(
                parent_kind,
                viewport,
                is_root,
                animations,
                theme,
                units,
                now,
            ),
        )?;
        if patched_layout_children.is_empty() {
            taffy.set_children(layout_node.node, &[])?;
            taffy.set_node_context(layout_node.node, Some(next.measure_context()))?;
        } else {
            let child_nodes = patched_layout_children
                .iter()
                .map(|child| child.node)
                .collect::<Vec<_>>();
            taffy.set_node_context(layout_node.node, None)?;
            taffy.set_children(layout_node.node, &child_nodes)?;
        }
        layout_node.children = patched_layout_children;
        Ok(next)
    })
}

fn remove_layout_subtree(
    taffy: &mut TaffyTree<MeasureContext>,
    layout_node: &LayoutNode,
) -> Result<(), taffy::TaffyError> {
    for child in &layout_node.children {
        remove_layout_subtree(taffy, child)?;
    }
    taffy.remove(layout_node.node)?;
    Ok(())
}

pub(super) fn resolved_at_path<'a, VM>(
    node: &'a ResolvedElement<VM>,
    path: &[usize],
) -> &'a ResolvedElement<VM> {
    if path.is_empty() {
        return node;
    }
    let children = match &node.kind {
        ResolvedWidgetKind::Container { children, .. }
        | ResolvedWidgetKind::Virtual { children, .. } => children,
        _ => panic!("resolved path descends into a non-container widget"),
    };
    resolved_at_path(&children[path[0]], &path[1..])
}

pub(super) fn patch_resolved_at_path<VM>(
    node: &mut ResolvedElement<VM>,
    path: &[usize],
    next: ResolvedElement<VM>,
) -> bool {
    if path.is_empty() {
        *node = next;
        return true;
    }

    let children = match &mut node.kind {
        ResolvedWidgetKind::Container { children, .. }
        | ResolvedWidgetKind::Virtual { children, .. } => children,
        _ => return false,
    };
    let Some(child) = children.get_mut(path[0]) else {
        return false;
    };
    patch_resolved_at_path(child, &path[1..], next)
}

pub(super) fn layout_at_path<'a>(node: &'a LayoutNode, path: &[usize]) -> &'a LayoutNode {
    if path.is_empty() {
        return node;
    }
    layout_at_path(&node.children[path[0]], &path[1..])
}

pub(in super::super) fn media_event_phase(
    loading: bool,
    error: Option<&str>,
) -> Option<MediaEventPhase> {
    if loading {
        Some(MediaEventPhase::Loading)
    } else if let Some(error) = error {
        Some(MediaEventPhase::Error(error.to_string()))
    } else {
        Some(MediaEventPhase::Success)
    }
}
