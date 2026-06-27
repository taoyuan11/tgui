use super::*;
use crate::log::{log_text_profile, text_profile_enabled};
use crate::ui::theme::StyleContext;
use crate::ui::widget::common::ChildSource;
use crate::ui::widget::r#virtual::{VirtualCacheState, VirtualViewportHint};
use crate::ui::widget::StyleSheet;
use std::borrow::Cow;
use std::time::Instant;

pub(super) fn resolved_child_elements_with_previous<'a, 'b, VM>(
    owner_id: WidgetId,
    child_sources: &'b [ChildSource<VM>],
    previous_children: &'a [ResolvedElement<VM>],
    child_source_spans: Option<&mut Vec<usize>>,
) -> Vec<(Cow<'b, Element<VM>>, Option<&'a ResolvedElement<VM>>)> {
    let previous_by_key: HashMap<_, _> = previous_children
        .iter()
        .filter_map(|child| child.key.as_ref().map(|key| (key.clone(), child)))
        .collect();
    let previous_by_id: HashMap<_, _> = previous_children
        .iter()
        .map(|child| (child.id, child))
        .collect();

    let mut resolved = Vec::new();
    let mut resolved_index = 0usize;
    let mut spans = child_source_spans;
    for child_source in child_sources {
        // Static children are borrowed straight from the source tree; only
        // dynamic resolvers produce freshly owned elements. The previous output's
        // id is carried over inside `resolve_with_previous`, so child ids no
        // longer need to be rewritten here.
        match child_source {
            ChildSource::Static(children) => {
                if let Some(spans) = spans.as_mut() {
                    spans.push(children.len());
                }
                resolved.extend(children.iter().map(|child| {
                    let previous_child = lookup_previous(
                        child,
                        &previous_by_key,
                        &previous_by_id,
                        previous_children.get(resolved_index),
                    );
                    resolved_index += 1;
                    (Cow::Borrowed(child), previous_child)
                }));
            }
            ChildSource::Dynamic(_) | ChildSource::KeyedFor(_) => {
                let source_children = child_source.resolve(Some(owner_id));
                if let Some(spans) = spans.as_mut() {
                    spans.push(source_children.len());
                }
                resolved.extend(source_children.into_iter().map(|child| {
                    let previous_child = lookup_previous(
                        &child,
                        &previous_by_key,
                        &previous_by_id,
                        previous_children.get(resolved_index),
                    );
                    resolved_index += 1;
                    (Cow::Owned(child), previous_child)
                }));
            }
            ChildSource::Show { .. } | ChildSource::Switch { .. } => {
                let source_children = child_source.resolve(Some(owner_id));
                if let Some(spans) = spans.as_mut() {
                    spans.push(source_children.len());
                }
                resolved.extend(source_children.into_iter().map(|child| {
                    let previous_child = lookup_previous(
                        &child,
                        &previous_by_key,
                        &previous_by_id,
                        previous_children.get(resolved_index),
                    );
                    resolved_index += 1;
                    (Cow::Owned(child), previous_child)
                }));
            }
        }
    }
    resolved
}

fn lookup_previous<'a, VM>(
    child: &Element<VM>,
    previous_by_key: &HashMap<WidgetKey, &'a ResolvedElement<VM>>,
    previous_by_id: &HashMap<WidgetId, &'a ResolvedElement<VM>>,
    previous_at_position: Option<&'a ResolvedElement<VM>>,
) -> Option<&'a ResolvedElement<VM>> {
    child
        .key
        .as_ref()
        .and_then(|key| previous_by_key.get(key).copied())
        .or_else(|| previous_by_id.get(&child.id).copied())
        .or_else(|| {
            child
                .key
                .is_none()
                .then_some(previous_at_position)
                .flatten()
        })
}

pub(super) fn resolve_subtree_from_source_path<'a, VM: 'static>(
    source: &Element<VM>,
    previous: Option<&'a ResolvedElement<VM>>,
    theme: &Theme,
    path: &[usize],
) -> Option<ResolvedElement<VM>> {
    super::tree::with_widget_stack_frame(|| {
        resolve_subtree_from_source_path_inner(source, previous, theme, path)
    })
}

fn resolve_subtree_from_source_path_inner<'a, VM: 'static>(
    source: &Element<VM>,
    previous: Option<&'a ResolvedElement<VM>>,
    theme: &Theme,
    path: &[usize],
) -> Option<ResolvedElement<VM>> {
    let started_at = text_profile_enabled().then_some(Instant::now());
    if path.is_empty() {
        let resolved = source.resolve_with_previous(theme, previous);
        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_patch_scene_resolve_roots",
                started_at.elapsed(),
                format!("path={:?} terminal=true widget_id={:?}", path, resolved.id),
            );
        }
        return Some(resolved);
    }

    let WidgetKind::Container { children, .. } = &source.kind else {
        return None;
    };
    let previous_children = previous
        .and_then(|previous| match &previous.kind {
            ResolvedWidgetKind::Container { children, .. } => Some(children.as_slice()),
            _ => None,
        })
        .unwrap_or(&[]);
    let owner_id = previous.map(|previous| previous.id).unwrap_or(source.id);
    let (source_index, local_index) = previous
        .and_then(|previous| previous.child_source_spans.get(..children.len()))
        .and_then(|spans| child_source_position(spans, path[0]))
        .or_else(|| child_source_position_from_source(children, owner_id, path[0]))?;
    let source_children = children.get(source_index)?.resolve(Some(owner_id));
    let resolved_children_len = source_children.len();
    let mut child = source_children.into_iter().nth(local_index)?;
    let previous_child = previous_children.get(path[0]);
    if let Some(previous_child) = previous_child {
        child.id = previous_child.id;
    }
    let resolved = resolve_subtree_from_source_path(&child, previous_child, theme, &path[1..]);
    if let Some(started_at) = started_at {
        log_text_profile(
            "textarea_patch_scene_resolve_roots",
            started_at.elapsed(),
            format!(
                "path={:?} owner_id={:?} source_index={} local_index={} resolved_children={} previous_children={}",
                path,
                owner_id,
                source_index,
                local_index,
                resolved_children_len,
                previous_children.len(),
            ),
        );
    }
    resolved
}

pub(super) fn resolve_subtree_from_source_path_with_runtime_state<'a, VM: 'static>(
    source: &Element<VM>,
    previous: Option<&'a ResolvedElement<VM>>,
    theme: &Theme,
    path: &[usize],
    scroll_offsets: &HashMap<WidgetId, Point>,
    virtual_states: &HashMap<WidgetId, VirtualCacheState>,
    fallback_viewport_hint: VirtualViewportHint,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
) -> Option<ResolvedElement<VM>> {
    super::tree::with_widget_stack_frame(|| {
        resolve_subtree_from_source_path_with_runtime_state_inner(
            source,
            previous,
            theme,
            path,
            scroll_offsets,
            virtual_states,
            fallback_viewport_hint,
            context,
            style_sheet,
        )
    })
}

fn resolve_subtree_from_source_path_with_runtime_state_inner<'a, VM: 'static>(
    source: &Element<VM>,
    previous: Option<&'a ResolvedElement<VM>>,
    theme: &Theme,
    path: &[usize],
    scroll_offsets: &HashMap<WidgetId, Point>,
    virtual_states: &HashMap<WidgetId, VirtualCacheState>,
    fallback_viewport_hint: VirtualViewportHint,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
) -> Option<ResolvedElement<VM>> {
    let started_at = text_profile_enabled().then_some(Instant::now());
    if path.is_empty() {
        let resolved = source.resolve_with_runtime_state_and_style_sheet(
            theme,
            previous,
            scroll_offsets,
            virtual_states,
            fallback_viewport_hint,
            context,
            style_sheet,
        );
        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_patch_scene_resolve_roots",
                started_at.elapsed(),
                format!("path={:?} terminal=true widget_id={:?}", path, resolved.id),
            );
        }
        return Some(resolved);
    }

    let WidgetKind::Container { children, .. } = &source.kind else {
        if matches!(source.kind, WidgetKind::Virtual { .. }) {
            let resolved = source.resolve_with_runtime_state_and_style_sheet(
                theme,
                previous,
                scroll_offsets,
                virtual_states,
                fallback_viewport_hint,
                context,
                style_sheet,
            );
            return cloned_resolved_subtree_at_path(&resolved, path);
        }
        return None;
    };
    let previous_children = previous
        .and_then(|previous| match &previous.kind {
            ResolvedWidgetKind::Container { children, .. } => Some(children.as_slice()),
            _ => None,
        })
        .unwrap_or(&[]);
    let owner_id = previous.map(|previous| previous.id).unwrap_or(source.id);
    let (source_index, local_index) = previous
        .and_then(|previous| previous.child_source_spans.get(..children.len()))
        .and_then(|spans| child_source_position(spans, path[0]))
        .or_else(|| child_source_position_from_source(children, owner_id, path[0]))?;
    let source_children = children.get(source_index)?.resolve(Some(owner_id));
    let resolved_children_len = source_children.len();
    let mut child = source_children.into_iter().nth(local_index)?;
    let previous_child = previous_children.get(path[0]);
    if let Some(previous_child) = previous_child {
        child.id = previous_child.id;
    }
    let resolved = resolve_subtree_from_source_path_with_runtime_state(
        &child,
        previous_child,
        theme,
        &path[1..],
        scroll_offsets,
        virtual_states,
        fallback_viewport_hint,
        context,
        style_sheet,
    );
    if let Some(started_at) = started_at {
        log_text_profile(
            "textarea_patch_scene_resolve_roots",
            started_at.elapsed(),
            format!(
                "path={:?} owner_id={:?} source_index={} local_index={} resolved_children={} previous_children={}",
                path,
                owner_id,
                source_index,
                local_index,
                resolved_children_len,
                previous_children.len(),
            ),
        );
    }
    resolved
}

fn child_source_position(spans: &[usize], child_index: usize) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    for (source_index, span) in spans.iter().copied().enumerate() {
        if child_index < offset + span {
            return Some((source_index, child_index - offset));
        }
        offset += span;
    }
    None
}

fn cloned_resolved_subtree_at_path<VM>(
    root: &ResolvedElement<VM>,
    path: &[usize],
) -> Option<ResolvedElement<VM>> {
    let mut current = root;
    for child_index in path {
        let children = match &current.kind {
            ResolvedWidgetKind::Container { children, .. }
            | ResolvedWidgetKind::Virtual { children, .. } => children,
            _ => return None,
        };
        current = children.get(*child_index)?;
    }
    Some(current.clone())
}

fn child_source_position_from_source<VM>(
    child_sources: &[ChildSource<VM>],
    owner_id: WidgetId,
    child_index: usize,
) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    for (source_index, child_source) in child_sources.iter().enumerate() {
        let span = child_source.resolve(Some(owner_id)).len();
        if child_index < offset + span {
            return Some((source_index, child_index - offset));
        }
        offset += span;
    }
    None
}
