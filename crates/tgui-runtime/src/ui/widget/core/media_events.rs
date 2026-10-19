use super::*;
use crate::ui::widget::common::{ChildSource, FocusScopeOptions};

/// A sparse subscription tree, built with the source widget tree. Visual-only branches never
/// participate in media polling. Gates are evaluated on every query (including unchecked
/// signals); dynamic sources and virtual items retain their original resolution semantics.
pub(super) enum MediaSubscriptions<VM> {
    Group(Vec<Self>),
    Scope(FocusScopeOptions, Box<Self>),
    Show(Value<bool>, Box<Self>),
    Switch {
        index: Value<usize>,
        cases: Vec<Option<Self>>,
        fallback: Option<Box<Self>>,
    },
    Tooltip(WidgetId, Box<Self>),
    Dynamic(WidgetId, ChildSource<VM>),
    Poll(Box<Element<VM>>),
}

impl<VM: 'static> MediaSubscriptions<VM> {
    pub(super) fn new(element: &Element<VM>) -> Option<Self> {
        // Virtual children depend on the runtime window plan. Resolve only this boundary,
        // using the existing event walk as a correctness fallback.
        if matches!(element.kind, WidgetKind::Virtual { .. }) {
            return Some(Self::Poll(Box::new(element.clone())));
        }
        let mut subscriptions = Vec::new();
        match &element.kind {
            WidgetKind::Container { children, .. } => {
                for source in children {
                    match source {
                        ChildSource::Static(children) => {
                            subscriptions.extend(children.iter().filter_map(Self::new));
                        }
                        ChildSource::Dynamic(_) | ChildSource::KeyedFor(_) => {
                            subscriptions.push(Self::Dynamic(element.id, source.clone()));
                        }
                        ChildSource::Show { visible, child } => {
                            if let Some(child) = Self::new(child) {
                                subscriptions.push(Self::Show(visible.clone(), Box::new(child)));
                            }
                        }
                        ChildSource::Switch {
                            index,
                            cases,
                            fallback,
                        } => {
                            let cases: Vec<_> = cases.iter().map(Self::new).collect();
                            let fallback = fallback.as_deref().and_then(Self::new).map(Box::new);
                            if cases.iter().any(Option::is_some) || fallback.is_some() {
                                subscriptions.push(Self::Switch {
                                    index: index.clone(),
                                    cases,
                                    fallback,
                                });
                            }
                        }
                    }
                }
            }
            WidgetKind::Portal { content, open, .. } => {
                if let Some(content) = Self::new(content) {
                    subscriptions.push(Self::Show(open.clone(), Box::new(content)));
                }
            }
            _ if element.media_events.has_any() => {
                // Keep the existing leaf snapshot logic for images, audio and video. Remove
                // the tooltip here because its subscription is gated separately below.
                let mut leaf = element.clone();
                leaf.tooltip = None;
                // The outer Scope node evaluates this gate once, including unchecked signals.
                leaf.focus.scope = None;
                subscriptions.push(Self::Poll(Box::new(leaf)));
            }
            _ => {}
        }
        if let Some(tooltip) = element.tooltip.as_ref() {
            if let crate::ui::widget::tooltip::TooltipContent::Element(content) = &tooltip.content {
                if let Some(content) = Self::new(content) {
                    subscriptions.push(Self::Tooltip(element.id, Box::new(content)));
                }
            }
        }
        let subscriptions = match subscriptions.len() {
            0 => return None,
            1 => subscriptions.pop().unwrap(),
            _ => Self::Group(subscriptions),
        };
        Some(match element.focus.scope.as_ref() {
            Some(scope) => Self::Scope(scope.clone(), Box::new(subscriptions)),
            None => subscriptions,
        })
    }

    pub(super) fn collect(
        &self,
        media: &MediaManager,
        theme: &Theme,
        active_tooltip: Option<WidgetId>,
        states: &mut Vec<MediaEventState<VM>>,
    ) {
        match self {
            Self::Group(children) => {
                for child in children {
                    child.collect(media, theme, active_tooltip, states);
                }
            }
            Self::Scope(scope, child) => {
                if !scope.suppresses_interactions(scope.is_active_untracked()) {
                    child.collect(media, theme, active_tooltip, states);
                }
            }
            Self::Show(visible, child) => {
                if visible.resolve() {
                    child.collect(media, theme, active_tooltip, states);
                }
            }
            Self::Switch {
                index,
                cases,
                fallback,
            } => {
                // An in-range empty branch must not select the fallback.
                let child = match cases.get(index.resolve()) {
                    Some(child) => child.as_ref(),
                    None => fallback.as_deref(),
                };
                if let Some(child) = child {
                    child.collect(media, theme, active_tooltip, states);
                }
            }
            Self::Tooltip(owner, child) => {
                if active_tooltip == Some(*owner) {
                    child.collect(media, theme, active_tooltip, states);
                }
            }
            Self::Dynamic(owner, source) => {
                for child in source.resolve(Some(*owner)) {
                    if let Some(subscription) = Self::new(&child) {
                        subscription.collect(media, theme, active_tooltip, states);
                    }
                }
            }
            Self::Poll(element) => {
                element.resolve(theme).collect_media_event_states(
                    media,
                    theme,
                    active_tooltip,
                    states,
                );
            }
        }
    }
}
