use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::StyleContext;
use crate::ui::layout::{pct, Axis, LayoutStyle, Value};
use crate::ui::theme::Theme;

use super::core::Element;
use super::p3_support::{impl_p3_layout_api, merge_layout};
use super::style::{ContainerStyle, SplitterStyle, StyleResolver};
use super::{CursorStyle, Flex, SplitterHandleState, Stack, WidgetKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitterAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SplitterResize {
    pub index: usize,
    pub sizes: Vec<f32>,
}

#[derive(Clone)]
pub struct Pane<VM> {
    content: Element<VM>,
    min: f32,
    max: f32,
}

impl<VM> Pane<VM> {
    pub fn new(content: impl Into<Element<VM>>) -> Self {
        Self {
            content: content.into(),
            min: 0.05,
            max: 1.0,
        }
    }

    pub fn min(mut self, min: f32) -> Self {
        self.min = min.clamp(0.0, 1.0);
        self
    }

    pub fn max(mut self, max: f32) -> Self {
        self.max = max.clamp(0.0, 1.0);
        self
    }
}

pub struct ResizablePanels<VM> {
    panes: Vec<Pane<VM>>,
    sizes: Value<Vec<f32>>,
    axis: SplitterAxis,
    step: f32,
    on_resize: Option<ValueCommand<VM, SplitterResize>>,
    style: Option<StyleResolver<SplitterStyle>>,
    layout: LayoutStyle,
    key: Option<WidgetKey>,
}

pub type Splitter<VM> = ResizablePanels<VM>;

impl<VM> ResizablePanels<VM> {
    pub fn new(panes: Vec<Pane<VM>>, sizes: impl Into<Value<Vec<f32>>>) -> Self {
        Self {
            panes,
            sizes: sizes.into(),
            axis: SplitterAxis::Horizontal,
            step: 0.05,
            on_resize: None,
            style: None,
            layout: LayoutStyle::default(),
            key: None,
        }
    }

    pub fn axis(mut self, axis: SplitterAxis) -> Self {
        self.axis = axis;
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = step.abs().max(0.001);
        self
    }

    pub fn on_resize(mut self, command: ValueCommand<VM, SplitterResize>) -> Self {
        self.on_resize = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut SplitterStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| SplitterStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> SplitterStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<ResizablePanels<VM>> for Element<VM> {
    fn from(splitter: ResizablePanels<VM>) -> Self {
        let layout_style = resolve_splitter_style_for_layout(splitter.style.as_ref());
        let constraints = splitter
            .panes
            .iter()
            .map(|pane| (pane.min, pane.max))
            .collect::<Vec<_>>();
        let sizes = normalize_sizes(splitter.sizes.resolve(), splitter.panes.len());
        let mut children = Vec::new();
        for (index, pane) in splitter.panes.into_iter().enumerate() {
            let mut content = pane.content;
            content.layout.grow = Value::Static(sizes[index].max(0.001));
            children.push(content);
            if index + 1 < sizes.len() {
                let on_resize = splitter.on_resize.clone();
                let next_sizes =
                    splitter_adjusted_sizes(&sizes, &constraints, index, splitter.step);
                let reset_sizes = splitter_reset_sizes(sizes.len());
                let style = splitter.style.clone();
                let axis = splitter.axis;
                let handle_width = if axis == SplitterAxis::Horizontal {
                    crate::ui::layout::Length::Px(layout_style.hit_extent)
                } else {
                    pct(100.0)
                };
                let handle_height = if axis == SplitterAxis::Horizontal {
                    pct(100.0)
                } else {
                    crate::ui::layout::Length::Px(layout_style.hit_extent)
                };
                let line_width = if axis == SplitterAxis::Horizontal {
                    crate::ui::layout::Length::Px(layout_style.handle_thickness)
                } else {
                    pct(100.0)
                };
                let line_height = if axis == SplitterAxis::Horizontal {
                    pct(100.0)
                } else {
                    crate::ui::layout::Length::Px(layout_style.handle_thickness)
                };
                let mut handle: Element<VM> = Stack::new()
                    .size(handle_width, handle_height)
                    .center()
                    .cursor(if axis == SplitterAxis::Horizontal {
                        CursorStyle::EwResize
                    } else {
                        CursorStyle::NsResize
                    })
                    .on_click(Command::new_with_context({
                        let on_resize = on_resize.clone();
                        let next_sizes = next_sizes.clone();
                        move |vm, context| {
                            if let Some(command) = on_resize.as_ref() {
                                command.execute_with_context(
                                    vm,
                                    SplitterResize {
                                        index,
                                        sizes: next_sizes.clone(),
                                    },
                                    context,
                                );
                            }
                        }
                    }))
                    .on_double_click(Command::new_with_context(move |vm, context| {
                        if let Some(command) = on_resize.as_ref() {
                            command.execute_with_context(
                                vm,
                                SplitterResize {
                                    index,
                                    sizes: reset_sizes.clone(),
                                },
                                context,
                            );
                        }
                    }))
                    .focusable(true)
                    .tab_index(0)
                    .child(Stack::<VM>::new().size(line_width, line_height).style_full(
                        move |context| {
                            let resolved = resolve_splitter_style(style.as_ref(), context);
                            let mut container = ContainerStyle::default_for_theme(context.theme);
                            container.surface.background =
                                Some(resolved.handle_color.normal.clone());
                            container
                        },
                    ))
                    .into();
                handle.splitter_handle = Some(SplitterHandleState {
                    axis: match axis {
                        SplitterAxis::Horizontal => Axis::Horizontal,
                        SplitterAxis::Vertical => Axis::Vertical,
                    },
                    index,
                    sizes: sizes.clone(),
                    constraints: constraints.clone(),
                    step: splitter.step,
                    on_resize: splitter.on_resize.clone(),
                });
                children.push(handle);
            }
        }
        let mut root: Element<VM> = Flex::new(match splitter.axis {
            SplitterAxis::Horizontal => Axis::Horizontal,
            SplitterAxis::Vertical => Axis::Vertical,
        })
        .gap(layout_style.gap)
        .child(children)
        .into();
        root.key = splitter.key;
        root.layout = merge_layout(root.layout, splitter.layout);
        root
    }
}

fn normalize_sizes(mut sizes: Vec<f32>, count: usize) -> Vec<f32> {
    if count == 0 {
        return Vec::new();
    }
    if sizes.len() != count
        || sizes
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        sizes = vec![1.0 / count as f32; count];
    }
    let total: f32 = sizes.iter().sum();
    if total <= f32::EPSILON {
        vec![1.0 / count as f32; count]
    } else {
        sizes.into_iter().map(|value| value / total).collect()
    }
}

pub(crate) fn splitter_adjusted_sizes(
    sizes: &[f32],
    constraints: &[(f32, f32)],
    index: usize,
    delta: f32,
) -> Vec<f32> {
    let mut next = sizes.to_vec();
    if index + 1 >= next.len() {
        return next;
    }
    if !delta.is_finite() || delta.abs() <= f32::EPSILON {
        return normalize_sizes(next, sizes.len());
    }
    let left_min = constraints.get(index).map(|(min, _)| *min).unwrap_or(0.0);
    let left_max = constraints.get(index).map(|(_, max)| *max).unwrap_or(1.0);
    let right_min = constraints
        .get(index + 1)
        .map(|(min, _)| *min)
        .unwrap_or(0.0);
    let right_max = constraints
        .get(index + 1)
        .map(|(_, max)| *max)
        .unwrap_or(1.0);
    if delta > 0.0 {
        let applied = delta
            .min(left_max - next[index])
            .min(next[index + 1] - right_min);
        if applied > 0.0 {
            next[index] += applied;
            next[index + 1] -= applied;
        }
    } else {
        let applied = (-delta)
            .min(next[index] - left_min)
            .min(right_max - next[index + 1]);
        if applied > 0.0 {
            next[index] -= applied;
            next[index + 1] += applied;
        }
    }
    normalize_sizes(next, sizes.len())
}

pub(crate) fn splitter_reset_sizes(count: usize) -> Vec<f32> {
    normalize_sizes(Vec::new(), count)
}

fn resolve_splitter_style(
    style: Option<&StyleResolver<SplitterStyle>>,
    context: &StyleContext<'_>,
) -> SplitterStyle {
    let mut base = SplitterStyle::default_for_theme(context.theme);
    context.theme.components.splitter.apply(&mut base, context);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

fn resolve_splitter_style_for_layout(
    style: Option<&StyleResolver<SplitterStyle>>,
) -> SplitterStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_splitter_style(style, &context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitter_adjusted_sizes_supports_positive_and_negative_delta() {
        let sizes = vec![0.5, 0.5];
        let constraints = vec![(0.2, 0.8), (0.2, 0.8)];

        assert_eq!(
            splitter_adjusted_sizes(&sizes, &constraints, 0, 0.1),
            vec![0.6, 0.4]
        );
        assert_eq!(
            splitter_adjusted_sizes(&sizes, &constraints, 0, -0.1),
            vec![0.4, 0.6]
        );
    }

    #[test]
    fn splitter_adjusted_sizes_respects_adjacent_min_max() {
        let sizes = vec![0.75, 0.25];
        let constraints = vec![(0.2, 0.8), (0.2, 0.8)];

        assert_eq!(
            splitter_adjusted_sizes(&sizes, &constraints, 0, 0.5),
            vec![0.8, 0.2]
        );
        assert_eq!(
            splitter_adjusted_sizes(&sizes, &constraints, 0, -0.8),
            vec![0.2, 0.8]
        );
    }
}
