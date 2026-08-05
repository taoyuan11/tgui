use super::*;
use crate::log::{log_text_profile, text_profile_enabled};
use crate::ui::theme::StyleContext;
use crate::ui::widget::common::ChildSource;
use crate::ui::widget::r#virtual::{VirtualCacheState, VirtualViewportHint};
use crate::ui::widget::StyleSheet;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

/// Resolves every child source in source order, then maps its children directly into the final
/// output buffer.
///
/// The source-resolution pass intentionally completes before `map_child` is called. Besides
/// preserving the existing dependency-read order, this gives the output an exact capacity. The
/// previous implementation first built a `Vec<(Cow<Element>, previous)>`; because `Cow<Element>`
/// is as wide as its large owned variant, a broad static container moved a substantial temporary
/// buffer before producing the identically sized final child vector.
#[inline]
pub(super) fn map_resolved_child_elements_with_previous<'a, VM, Output>(
    owner_id: WidgetId,
    child_sources: &[ChildSource<VM>],
    previous_children: &'a [ResolvedElement<VM>],
    child_source_spans: Option<&mut Vec<usize>>,
    mut map_child: impl FnMut(&Element<VM>, Option<&'a ResolvedElement<VM>>) -> Output,
) -> Vec<Output> {
    let keyed_previous_count = previous_children
        .iter()
        .filter(|child| child.key.is_some())
        .count();
    let mut previous_by_key = HashMap::with_capacity(keyed_previous_count);
    for child in previous_children {
        if let Some(key) = child.key.as_ref() {
            previous_by_key
                .entry(key)
                .or_insert_with(VecDeque::new)
                .push_back(child);
        }
    }
    let previous_by_id: HashMap<_, _> = previous_children
        .iter()
        .map(|child| (child.id, child))
        .collect();

    // Static sources are already retained in the source tree. Only keep the owning results needed
    // for non-static sources between the source-resolution and child-mapping passes. This small
    // source-level buffer preserves the old "resolve all sources, then recurse" ordering without a
    // per-child `Cow<Element>` buffer.
    let owned_source_count = child_sources
        .iter()
        .filter(|source| !matches!(source, ChildSource::Static(_)))
        .count();
    let mut owned_sources = Vec::with_capacity(owned_source_count);
    let mut child_count = 0usize;
    let mut spans = child_source_spans;
    for child_source in child_sources {
        let span = match child_source {
            ChildSource::Static(children) => children.len(),
            ChildSource::Dynamic(_)
            | ChildSource::KeyedFor(_)
            | ChildSource::Show { .. }
            | ChildSource::Switch { .. } => {
                let children = child_source.resolve(Some(owner_id));
                let span = children.len();
                owned_sources.push(children);
                span
            }
        };
        child_count += span;
        if let Some(spans) = spans.as_mut() {
            spans.push(span);
        }
    }

    let mut output = Vec::with_capacity(child_count);
    let mut owned_sources = owned_sources.into_iter();
    let mut resolved_index = 0usize;
    let mut reused_previous_ids = HashSet::with_capacity(previous_children.len());
    for child_source in child_sources {
        match child_source {
            ChildSource::Static(children) => {
                for child in children {
                    let previous_child = lookup_previous(
                        child,
                        &mut previous_by_key,
                        &previous_by_id,
                        previous_children.get(resolved_index),
                        &mut reused_previous_ids,
                    );
                    resolved_index += 1;
                    output.push(map_child(child, previous_child));
                }
            }
            ChildSource::Dynamic(_)
            | ChildSource::KeyedFor(_)
            | ChildSource::Show { .. }
            | ChildSource::Switch { .. } => {
                let children = owned_sources
                    .next()
                    .expect("every non-static child source must have a resolved batch");
                for child in children {
                    let previous_child = lookup_previous(
                        &child,
                        &mut previous_by_key,
                        &previous_by_id,
                        previous_children.get(resolved_index),
                        &mut reused_previous_ids,
                    );
                    resolved_index += 1;
                    output.push(map_child(&child, previous_child));
                }
            }
        }
    }
    debug_assert!(owned_sources.next().is_none());
    debug_assert_eq!(output.len(), child_count);
    output
}

fn lookup_previous<'a, VM>(
    child: &Element<VM>,
    previous_by_key: &mut HashMap<&WidgetKey, VecDeque<&'a ResolvedElement<VM>>>,
    previous_by_id: &HashMap<WidgetId, &'a ResolvedElement<VM>>,
    previous_at_position: Option<&'a ResolvedElement<VM>>,
    reused_previous_ids: &mut HashSet<WidgetId>,
) -> Option<&'a ResolvedElement<VM>> {
    let matching_id = previous_by_id
        .get(&child.id)
        .copied()
        .filter(|previous| previous.key == child.key)
        .filter(|previous| reused_previous_ids.insert(previous.id));
    if matching_id.is_some() {
        return matching_id;
    }

    if let Some(key) = child.key.as_ref() {
        if let Some(candidates) = previous_by_key.get_mut(key) {
            while let Some(previous) = candidates.pop_front() {
                if reused_previous_ids.insert(previous.id) {
                    return Some(previous);
                }
            }
        }
        return None;
    }

    previous_at_position.filter(|previous| reused_previous_ids.insert(previous.id))
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
