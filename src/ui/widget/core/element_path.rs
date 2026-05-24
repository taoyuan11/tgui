use super::*;
use crate::log::{log_text_profile, text_profile_enabled};
use crate::ui::widget::common::ChildSource;
use std::time::Instant;

pub(super) fn resolved_child_elements_with_previous<'a, VM>(
    owner_id: WidgetId,
    child_sources: &[ChildSource<VM>],
    previous_children: &'a [ResolvedElement<VM>],
    child_source_spans: Option<&mut Vec<usize>>,
) -> Vec<(Element<VM>, Option<&'a ResolvedElement<VM>>)> {
    let previous_by_key: HashMap<_, _> = previous_children
        .iter()
        .filter_map(|child| child.key.as_ref().map(|key| (key.clone(), child)))
        .collect();
    let previous_by_id: HashMap<_, _> = previous_children
        .iter()
        .map(|child| (child.id, child))
        .collect();

    let mut resolved = Vec::new();
    let mut spans = child_source_spans;
    for child_source in child_sources {
        let source_children = child_source.resolve(Some(owner_id));
        if let Some(spans) = spans.as_mut() {
            spans.push(source_children.len());
        }
        resolved.extend(source_children.into_iter().map(|mut child| {
            let previous_child = child
                .key
                .as_ref()
                .and_then(|key| previous_by_key.get(key).copied())
                .or_else(|| previous_by_id.get(&child.id).copied());
            if let Some(previous_child) = previous_child {
                child.id = previous_child.id;
            }
            (child, previous_child)
        }));
    }
    resolved
}

pub(super) fn resolve_subtree_from_source_path<'a, VM: 'static>(
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
